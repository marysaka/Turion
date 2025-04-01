import ftplib
import ssl

from typing import Union, IO, Any
from pathlib import Path


# From https://stackoverflow.com/a/36049814
class ImplicitFTP_TLS(ftplib.FTP_TLS):
    """FTP_TLS subclass that automatically wraps sockets in SSL to support implicit FTPS."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._sock = None

    @property
    def sock(self):
        """Return the socket."""
        return self._sock

    @sock.setter
    def sock(self, value):
        """When modifying the socket, ensure that it is ssl wrapped."""
        if value is not None and not isinstance(value, ssl.SSLSocket):
            value = self.context.wrap_socket(value)
        self._sock = value


class BambuSFTP(object):
    sftp: ImplicitFTP_TLS
    host: str
    port: int
    user: str
    pwd: str

    def __init__(self, host: str, port: int, user: str, pwd: str):
        self.host = host
        self.port = port
        self.user = user
        self.pwd = pwd

        self.sftp = ImplicitFTP_TLS()

    def connect(self):
        self.sftp.connect(self.host, self.port)
        self.sftp.login(self.user, self.pwd)
        self.sftp.prot_p()
        self.sftp.set_pasv(True)

    def disconnect(self):
        self.sftp.quit()

    def __enter__(self):
        self.connect()
        return self

    def __exit__(self, exception_type, exception_value, exception_traceback):
        self.disconnect()

    def delete(self, file_name: str) -> bool:
        try:
            self.sftp.delete(file_name)
            return True
        except Exception:
            return False

    def store_file(self, file_name: str, fp: Union[Path, IO[Any]]):
        self.sftp.voidcmd("TYPE I")

        with self.sftp.transfercmd(f"STOR {file_name}", None) as conn:
            while buf := fp.read(0x10000):
                conn.sendall(buf)

        self.sftp.voidresp()
