from bambu_mqtt import BambuMQTT
from bambu_sftp import BambuSFTP
from pathlib import Path
from typing import Dict
import argparse
import sys
import time
import os

parser = argparse.ArgumentParser(
    prog="turion_print_3mf",
    description="Simple script to upload and print a 3MF project",
)
parser.add_argument("host")
parser.add_argument("username")
parser.add_argument("password")
parser.add_argument("project_file_path")
parser.add_argument("--use-ams", action="store_true")
parser.add_argument("--ams-mapping", nargs="+", type=int)
parser.add_argument("--debug", action="store_true")

args = parser.parse_args()

project_file_path = args.project_file_path
project_printer_path = os.path.basename(project_file_path)
project_printer_url = f"ftp://sdcard/{project_printer_path}"
ams_mapping = args.ams_mapping

# Default an AMS mapping if it should be in use
if args.use_ams and not ams_mapping:
    ams_mapping = [0, 1, 2, 3]
elif not ams_mapping:
    ams_mapping = list()


print(f"Uploading {project_file_path}")
with BambuSFTP(args.host, 990, args.username, args.password) as printer_sftp:
    # First we upload the project file
    printer_sftp.delete(project_printer_path)

    with open(project_file_path, "rb") as f:
        printer_sftp.store_file(project_printer_path, f)

printer_mqtt = BambuMQTT(
    args.host, 8883, args.username, args.password, debug=args.debug
)
printer_mqtt.connect()

should_track_state = True
exit_code = 0


def printer_state_tracker(res: Dict[str, object], mqtt: BambuMQTT):
    global should_track_state
    global exit_code

    print_object = res.get("print")

    if not print_object:
        return

    gcode_state = print_object.get("gcode_state")
    print_error = print_object.get("print_error", 0)
    mc_percent = print_object.get("mc_percent")

    if gcode_state:
        print(f"State: {gcode_state}")

    if mc_percent:
        print(f"Print progression: {mc_percent}%")

    # We got an error stop the print and let's exit
    if print_error != 0:
        print(f"PRINT ERROR: {print_error:x}")
        print("Stopping print and exiting")
        mqtt.stop_print_no_reply()
        should_track_state = False
        exit_code = 1

    if gcode_state == "FINISH":
        print("Print completed")
        should_track_state = False
        exit_code = 0

    if not should_track_state:
        printer_mqtt.set_push_status_update_callback(None, None)


printer_mqtt.set_push_status_update_callback(printer_state_tracker, printer_mqtt)

print(f"Starting print of {project_printer_url}")
res = printer_mqtt.print_project(project_printer_url, ams_mapping)

if res["print"]["result"] == "success":
    print("Print successfully started")
else:
    # Force stop the print
    printer_mqtt.stop_print()
    reason = res["print"]["reason"]
    print(f"Print error: {reason}")
    should_track_state = False
    printer_mqtt.disconnect()
    sys.exit(1)

while should_track_state:
    time.sleep(1)

printer_mqtt.disconnect()
sys.exit(exit_code)
