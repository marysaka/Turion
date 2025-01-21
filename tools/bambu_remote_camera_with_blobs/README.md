# bambu_remote_camera_with_blobs

Some utility that use `libBambuSource.so` and output Bambu Labs 3D printer camera stream on **stderr** (For some reason Bambu Labs proprietary library output some debug information on stdout)

On a P1S, this output a [MJPG](https://en.wikipedia.org/wiki/Motion_JPEG) stream.

## Usage

```bash
export LIB_BAMBU_SOURCE_PATH="<libBambuSource.so full path>"
export IP="<IP of the printer>"
export PASS="<access code>"

./bambu_remote_camera_with_blobs "$LIB_BAMBU_SOURCE_PATH" "bambu:///local/$IP.?port=6000&user=bblp&passwd=$PASS" 2>&1 > /dev/null
```

## Licensing

This software is licensed under the terms of the AGPLv3.

You can find a copy of the license in the [LICENSE file](LICENSE).
