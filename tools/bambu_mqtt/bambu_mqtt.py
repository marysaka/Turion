from cryptography import x509
from cryptography.hazmat.backends import default_backend
from cryptography.x509.oid import NameOID
from typing import Callable, Optional, List, Dict
import paho.mqtt.client as mqtt
import ssl
import json
import queue
import time
import os


# From https://github.com/Doridian/OpenBambuAPI/blob/920f7d580889092a4bef02dfe02e0cc3123cc0ce/examples/mqtt.py
class MQTTSClient(mqtt.Client):
    """
    MQTT Client that supports custom certificate Server Name Indication (SNI) for TLS.
    see https://github.com/eclipse-paho/paho.mqtt.python/issues/734#issuecomment-2256633060
    """

    def __init__(self, *args, server_name=None, **kwargs):
        super().__init__(*args, **kwargs)
        self._server_name = server_name

    def _ssl_wrap_socket(self, tcp_sock) -> ssl.SSLSocket:
        orig_host = self._host
        if self._server_name:
            self._host = self._server_name
        res = super()._ssl_wrap_socket(tcp_sock)
        self._host = orig_host
        return res


class BambuMQTT(object):
    mqttc: mqtt.Client
    host: str
    port: int
    user: str
    pwd: str
    serial_number: Optional[str]
    is_connecting: bool
    conn_state: queue.Queue
    internal_queue: queue.Queue
    debug: bool

    push_status_update_callback: Optional[Callable[Dict[str, object], object]]
    push_status_update_userdata: Optional[object]
    sequence_id: int

    def __init__(self, host: str, port: int, user: str, pwd: str, debug: bool = False):
        self.host = host
        self.port = port
        self.user = user
        self.pwd = pwd
        self.debug = debug
        self.is_connecting = False
        self.conn_state = queue.Queue(maxsize=1)
        self.internal_queue = queue.Queue()

        ssl_context = ssl.create_default_context(cafile="ca_cert.pem")
        ssl_context.verify_flags &= ~ssl.VERIFY_X509_STRICT

        # We grab the device_id from the cert, should be fine (as we use the CA)
        self.serial_number = BambuMQTT.probe_serial_number(self.host, self.port)
        self.mqttc = MQTTSClient(
            mqtt.CallbackAPIVersion.VERSION2, server_name=self.serial_number
        )
        self.mqttc.tls_set(tls_version=ssl.PROTOCOL_TLS, cert_reqs=ssl.CERT_NONE)
        self.mqttc.user_data_set(self)
        self.mqttc.reconnect_delay_set(min_delay=1, max_delay=1)
        self.mqttc.on_connect = self.__mqttc_on_connect
        self.mqttc.on_message = self.__mqttc_on_message
        self.sequence_id = 0
        self.push_status_update_callback = None
        self.push_status_update_userdata = None

    def set_push_status_update_callback(
        self,
        callback: Optional[Callable[Dict[str, object], object]],
        userdata: Optional[object],
    ):
        self.push_status_update_callback = callback
        self.push_status_update_userdata = userdata

    def __get_next_sequence_id(self) -> int:
        val = self.sequence_id
        self.sequence_id += 1

        return val

    @staticmethod
    def __mqttc_on_connect(
        client: mqtt.Client, userdata: "BambuMQTT", flags, reason_code, properties
    ):
        if userdata.debug:
            print(f"DEBUG: Connected with result code {reason_code}")
        client.subscribe(f"device/{userdata.serial_number}/report")

    @staticmethod
    def __mqttc_on_message(client: mqtt.Client, userdata: "BambuMQTT", msg):

        payload = json.loads(msg.payload.decode("utf-8"))

        if userdata.debug:
            print(f"DEBUG: Received payload: {payload}")

        is_push_status = (
            payload.get("print") and payload["print"].get("command") == "push_status"
        )

        if is_push_status and userdata.is_connecting:
            userdata.conn_state.put(True)
            userdata.is_connecting = False

        if not is_push_status:
            userdata.internal_queue.put(payload)
        elif userdata.push_status_update_callback:
            userdata.push_status_update_callback(
                payload, userdata.push_status_update_userdata
            )

    def connect(self):
        self.is_connecting = True
        self.mqttc.username_pw_set(self.user, password=self.pwd)
        self.mqttc.connect(self.host, self.port, keepalive=60)
        self.mqttc.loop_start()
        self.conn_state.get()

    def disconnect(self):
        self.mqttc.disconnect()
        self.mqttc.loop_stop()

    def __enter__(self):
        self.connect()
        return self

    def __exit__(self, exception_type, exception_value, exception_traceback):
        self.disconnect()

    @staticmethod
    def probe_serial_number(host: str, port: int) -> str:
        # We need the serial number to do most operations, luckily for us the server cert contains it as CN.
        raw_cert: str = ssl.get_server_certificate((host, port))
        cert = x509.load_pem_x509_certificate(
            raw_cert.encode("utf-8"), default_backend()
        )
        serial_number = cert.subject.get_attributes_for_oid(NameOID.COMMON_NAME)[
            0
        ].value
        return serial_number

    def publish(self, message: object):
        self.mqttc.publish(f"device/{self.serial_number}/request", json.dumps(message))

    def publish_with_reply(self, message: object) -> object:
        self.publish(message)

        res = self.internal_queue.get()
        self.internal_queue.task_done()

        return res

    def run_raw_gcode(self, gcode: str) -> object:
        msg = {
            "print": {
                "sequence_id": str(self.__get_next_sequence_id()),
                "command": "gcode_line",
                "param": gcode,
                "user_id": "0",
            },
        }

        return self.publish_with_reply(msg)

    def print_gcode(self, url: str) -> object:
        msg = {
            "print": {
                "sequence_id": str(self.__get_next_sequence_id()),
                "command": "gcode_file",
                "param": url,
            },
        }

        return self.publish_with_reply(msg)

    def print_project(
        self,
        url: str,
        ams_mapping: List[int],
        plate_id: int = 1,
        task_name: str = None,
        timelapse: bool = True,
        bed_levelling: bool = True,
        flow_calibration: bool = True,
        vibration_calibration: bool = True,
        layer_inspect: bool = True,
    ) -> object:
        if not task_name:
            task_name = os.path.basename(url)

        msg = {
            "print": {
                "sequence_id": str(self.__get_next_sequence_id()),
                "command": "project_file",
                "param": f"Metadata/plate_{plate_id}.gcode",
                "project_id": "0",
                "profile_id": "0",
                "task_id": "0",
                "subtask_id": "0",
                "subtask_name": "",
                "file": "",
                "url": url,
                "md5": "",
                "timelapse": timelapse,
                "bed_type": "textured_plate",
                "bed_levelling": bed_levelling,
                "flow_cali": flow_calibration,
                "vibration_cali": vibration_calibration,
                "layer_inspect": layer_inspect,
                "ams_mapping": ams_mapping,
                "use_ams": len(ams_mapping) > 0,
            },
        }

        return self.publish_with_reply(msg)

    def stop_print_no_reply(self, with_reply: bool = True) -> object:
        msg = {
            "print": {
                "sequence_id": str(self.__get_next_sequence_id()),
                "command": "stop",
                "param": "",
            },
        }

        self.publish(msg)

    def stop_print(self) -> object:
        msg = {
            "print": {
                "sequence_id": str(self.__get_next_sequence_id()),
                "command": "stop",
                "param": "",
            },
        }

        return self.publish_with_reply(msg)

    def pause_print(self) -> object:
        msg = {
            "print": {
                "sequence_id": str(self.__get_next_sequence_id()),
                "command": "pause",
                "param": "",
            },
        }

        return self.publish_with_reply(msg)

    def resume_print(self) -> object:
        msg = {
            "print": {
                "sequence_id": str(self.__get_next_sequence_id()),
                "command": "resume",
                "param": "",
            },
        }

        return self.publish_with_reply(msg)
