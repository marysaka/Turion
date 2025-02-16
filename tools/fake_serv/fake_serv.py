import argparse, socket, ssl, os

parser = argparse.ArgumentParser(
    prog="fake_serv",
    description="Some simple MITM server",
)
parser.add_argument("host")
parser.add_argument("--port", type=int, default=6000)
parser.add_argument("--pem-path", type=str, default="dummy.pem")
args = parser.parse_args()


server_ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
server_ctx.minimum_version = ssl.TLSVersion.TLSv1_2
server_ctx.maximum_version = ssl.TLSVersion.TLSv1_2
server_ctx.load_cert_chain(certfile=args.pem_path, keyfile=args.pem_path)

client_ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
client_ctx.minimum_version = ssl.TLSVersion.TLSv1_2
client_ctx.maximum_version = ssl.TLSVersion.TLSv1_2
client_ctx.check_hostname = False
client_ctx.verify_mode = ssl.CERT_NONE

os.makedirs("packets", exist_ok=True)

bindsocket = socket.socket()
bindsocket.bind(("", args.port))
bindsocket.listen(5)


def dump_data(direction: str, data: bytes, idx: int):
    with open(f"packets/{idx}_{direction}.bin", "wb") as f:
        f.write(data)


while True:
    newsocket, fromaddr = bindsocket.accept()
    connstream = server_ctx.wrap_socket(newsocket, server_side=True)
    connstream.setblocking(False)

    idx = 0

    with socket.create_connection((args.host, args.port)) as sock:
        with client_ctx.wrap_socket(sock) as ssock:
            ssock.setblocking(False)
            while True:
                # read client data
                try:
                    client_data = connstream.recv()
                    if len(client_data) != 0:
                        dump_data("client", client_data, idx)
                        idx += 1
                    ssock.send(client_data)
                except ssl.SSLWantReadError:
                    pass

                try:
                    server_data = ssock.recv()
                    if len(server_data) != 0:
                        dump_data("server", server_data, idx)
                        idx += 1
                    connstream.send(server_data)
                except ssl.SSLWantReadError:
                    pass
