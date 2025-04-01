# bambu_mqtt

Some Python 3 utility that allow printing with Bambu Labs 3D printer MQTT / FTP.

## Example

You can use `turion_print_3mf.py` to test printing a 3MF project with embedded GCODE: ("Export plate sliced file" in OrcaSlicer)

```bash
export IP="<IP of the printer>"
export PASS="<access code>"

python3 turion_print_3mf.py $IP bblp $PASS ./sample.gcode.3mf
```

