#!/usr/bin/python3
# SPDX-FileCopyrightText: Copyright 2025 Mary Guillemard
# SPDX-License-Identifier: GPL-3.0

import socket
import ssl
import argparse
import struct
import sys

context = ssl.create_default_context()
context.check_hostname = False
context.verify_mode = ssl.CERT_NONE

BAMBU_CAMERA_STREAM = 0x3000
BAMBU_CAMERA_START = 0x40
BAMBU_CAMERA_STOP = 0x00


def generate_packet(username: str, password: str, word0: int, word1: int) -> bytes:
    res = struct.pack(
        "IIxxxxxxxx32s32s",
        word0,
        word1,
        username.encode("utf8"),
        password.encode("utf8"),
    )
    return res


def recv_full_size(sock: ssl.SSLSocket, size: int) -> bytes:
    data = b""

    while len(data) != size:
        frame = sock.read(len=(size - len(data)))

        data += frame

    return data


def handle_packets(sock: ssl.SSLSocket):
    data = recv_full_size(sock, 0x10)
    (data_size, unk1, unk2, unk3) = struct.unpack("IIII", data)

    frame = recv_full_size(sock, data_size)
    sys.stdout.buffer.write(frame)
    sys.stdout.buffer.flush()


parser = argparse.ArgumentParser(
    prog="bambu_camera_access",
    description="Access Bambu Camera and output raw stream on stdout",
)
parser.add_argument("host")
parser.add_argument("username")
parser.add_argument("password")

args = parser.parse_args()

with socket.create_connection((args.host, 6000)) as sock:
    with context.wrap_socket(sock) as ssock:
        ssock.write(
            generate_packet(
                args.username, args.password, BAMBU_CAMERA_START, BAMBU_CAMERA_STREAM
            )
        )

        while True:
            handle_packets(ssock)
