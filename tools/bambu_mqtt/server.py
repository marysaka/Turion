from bambu_mqtt import BambuMQTT
from bambu_sftp import BambuSFTP

from typing import Optional, Dict

import json
import tornado.httpserver, tornado.web
import os
import time


def parse_x_api_key(raw: str) -> Optional[Dict[str, str]]:
    res = dict()

    for raw_entry in raw.split(";"):
        parts = raw_entry.split("=")

        if len(parts) != 2:
            return None

        res[parts[0]] = parts[1]

    if not res.get("host") or not res.get("pass"):
        return None

    res["user"] = res.get("user", "bblp")
    res["timelapse"] = res.get("timelapse", "false") == "true"
    res["bed_type"] = res.get("bed_type", "auto")
    res["bed_levelling"] = res.get("bed_levelling", "true") == "true"
    res["flow_calibration"] = res.get("flow_calibration", "true") == "true"
    res["vibration_calibration"] = res.get("vibration_calibration", "true") == "true"
    res["layer_inspect"] = res.get("layer_inspect", "true") == "true"
    tmp_ams_mapping = res.get("ams_mapping", "").split(",")

    res["ams_mapping"] = list()
    for entry in tmp_ams_mapping:
        res["ams_mapping"].append(int(entry))

    return res


class BaseRequestHandler(tornado.web.RequestHandler):
    def prepare(self):
        if "X-Api-Key" not in self.request.headers:
            raise tornado.web.HTTPError(401, "Missing X-Api-Key")

        self.serv_config = parse_x_api_key(self.request.headers["X-Api-Key"])

        if not self.serv_config:
            raise tornado.web.HTTPError(401)


class IndexHandler(BaseRequestHandler):
    def get(self):
        self.write(
            {
                "api": "0.1",
                "server": "1.3.10",
                # Need to start with "OctoPrint" for OrcaSlicer to understand us...
                "text": "OctoPrint Compatible Turion Link 0.0.1",
            }
        )


class PrintHandler(BaseRequestHandler):
    def post(self):
        command = self.get_body_argument("command", "select")

        # We implement the minimal subset needed by OrcaSlicer
        if command != "select":
            self.write_error(503)
            return

        print_file = self.request.arguments.get("print", [b"false"])[-1] == b"true"
        upload_path = self.request.arguments.get("path", [b""])[-1].decode("utf-8")

        if len(self.request.files) == 0:
            self.write_error(400)
            return

        first_entry_key = next(iter(self.request.files))
        file_info = self.request.files[first_entry_key][0]
        file_name: str = file_info["filename"]
        file_data = file_info["body"]

        with open(file_name, "wb") as f:
            f.write(file_data)

        # We only support 3MF with embedded GCODE
        if not file_name.endswith(".3mf"):
            self.write_error(503)
            return

        project_printer_path = os.path.basename(file_name)

        host = self.serv_config["host"]
        username = self.serv_config["user"]
        password = self.serv_config["pass"]
        timelapse = self.serv_config["timelapse"]
        bed_type = self.serv_config["bed_type"]
        bed_levelling = self.serv_config["bed_levelling"]
        flow_calibration = self.serv_config["flow_calibration"]
        vibration_calibration = self.serv_config["vibration_calibration"]
        layer_inspect = self.serv_config["layer_inspect"]
        ams_mapping = self.serv_config["ams_mapping"]

        if upload_path.startswith("/"):
            project_printer_uri = f"file:///sdcard{upload_path}/{file_name}"
        else:
            project_printer_uri = f"file:///sdcard/{upload_path}/{file_name}"

        print(f"Uploading {project_printer_uri}")
        with BambuSFTP(host, 990, username, password) as printer_sftp:
            printer_sftp.enter_create_directories(upload_path)
            # First we upload the project file
            printer_sftp.delete(project_printer_path)
            printer_sftp.store_data(project_printer_path, file_data)
        print(f"Uploaded {project_printer_uri}")

        if print_file:
            with BambuMQTT(host, 8883, username, password, debug=False) as mqtt:
                res = mqtt.print_project(
                    project_printer_uri,
                    ams_mapping,
                    1,
                    f"{file_name} (via TurionLink)",
                    timelapse,
                    bed_type,
                    bed_levelling,
                    flow_calibration,
                    vibration_calibration,
                    layer_inspect,
                )

                if res["print"]["result"] != "success":
                    self.write_error(419)
                    return

        self.set_status(204)


if __name__ == "__main__":
    application = tornado.web.Application(
        [(r"/api/version", IndexHandler), (r"/api/files/local", PrintHandler)],
        debug=False,
    )

    application.listen(9931)
    tornado.ioloop.IOLoop.instance().start()
