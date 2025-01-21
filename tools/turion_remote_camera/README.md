# turion_remote_camera

Some Python 3 utility that output Bambu Labs 3D printer camera steram on **stdout**.

On a P1S, this output a [MJPG](https://en.wikipedia.org/wiki/Motion_JPEG) stream.

## Usage

```bash
export IP="<IP of the printer>"
export PASS="<access code>"

python3 turion_remote_camera.py "$IP" "bblp" "$PASS"
```

You can also get some simple stream server up using [go2rtc](https://github.com/AlexxIT/go2rtc) and the configuration provided.

