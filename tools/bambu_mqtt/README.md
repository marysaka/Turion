# bambu_mqtt

Some Python 3 utility that allow printing with Bambu Labs 3D printer MQTT / FTP.

## turion_print_3mf.py

You can use `turion_print_3mf.py` to test printing a 3MF project with embedded GCODE: ("Export plate sliced file" in OrcaSlicer)

**WARN**: sample.gcode.3mf was generated with P1S 0.4 nuzzle profile.

```bash
export IP="<IP of the printer>"
export PASS="<access code>"

python3 turion_print_3mf.py $IP bblp $PASS ./sample.gcode.3mf
```

## server.py

This is a minimal server that simulate an OctoPrint server enough to make OrcaSlicer upload and print a file. Printer configuration is done via the "API Key" field in OrcaSlicer.

To run the server you need Tornado (`pip install tornado`)

On OrcaSlicer side, select the preset you would normally use, click on the "edit preset" button and toggle on the "Use 3rd party print host" option.

After that a new wireless icon will show up, click on it, set the host field to "127.0.0.1:9931" and the "API Key" field with `host=<IP of the printer>;pass=<access code>` (In case you use an AMS, you need to add the appropriate like `;ams_mapping=0,1,2,3`)

API Key example for a P1S with AMS identity mapping and timelapse enabled: `host=<IP of the printer>;pass=<access code>;timelapse=true;ams_mapping=0,1` (see the source code for all options)

If everything is okay, you can now just "Print plate".
