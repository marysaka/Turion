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


def generate_header_packet(
    size: int, packet_type: int, flags: int, word3: int
) -> bytes:
    res = struct.pack(
        "IIII",
        size,
        packet_type,
        flags,
        word3,
    )
    return res


def generate_login_packet(
    username: str,
    password: str,
) -> bytes:
    res = generate_header_packet(0x40, BAMBU_CAMERA_STREAM, 0, 0)
    res += struct.pack(
        "32s32s",
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
        ssock.write(generate_login_packet(args.username, args.password))

        while True:
            handle_packets(ssock)
