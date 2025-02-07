// Copyright 2025 Mary Guillemard
// SPDX-License-Identifier: LGPL-3.0

mod api;

use std::io::{self, Error, ErrorKind, Read, Write};
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use rustls::client::danger::HandshakeSignatureValid;
use rustls::crypto::{aws_lc_rs as provider, CryptoProvider};
use rustls::pki_types::{CertificateDer, UnixTime};
use rustls::DigitallySignedStruct;
use rustls::{pki_types::ServerName, RootCertStore};
use thiserror::Error;

use mio::net::TcpStream;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("Local settings parsing: {0}")]
pub struct LocalSettingsParsingError(&'static str);

#[derive(Clone, Debug)]
pub struct LocalSettings {
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub password: String,

    pub serial: Option<String>,
    pub net_ver: Option<String>,
    pub dev_ver: Option<String>,
    pub cli_id: Option<String>,
    pub cli_ver: Option<String>,
}

const SCHEMA_START: &str = "bambu:///local/";

impl LocalSettings {
    pub fn from_url(url: &str) -> Result<Self> {
        if !url.starts_with(SCHEMA_START) {
            bail!(LocalSettingsParsingError("Invalid schema"))
        }

        let part = &url[SCHEMA_START.len()..];
        let ip_end = match part.find(".?") {
            Some(ip_end) => ip_end,
            None => bail!(LocalSettingsParsingError("Invalid url")),
        };

        let hostname: String = part[0..ip_end].to_string();
        let raw_query = &part[ip_end + 2..];

        let mut username = None;
        let mut password = None;
        let mut port = None;
        let mut serial = None;
        let mut net_ver = None;
        let mut dev_ver = None;
        let mut cli_id = None;
        let mut cli_ver = None;

        for raw_key_val in raw_query.split("&") {
            let mut parts = raw_key_val.split("=");
            let key = parts.next().unwrap();
            let val = parts.next().unwrap();

            match key {
                "user" => username = Some(val.to_string()),
                "passwd" => password = Some(val.to_string()),
                "device" => serial = Some(val.to_string()),
                "net_ver" => net_ver = Some(val.to_string()),
                "dev_ver" => dev_ver = Some(val.to_string()),
                "cli_id" => cli_id = Some(val.to_string()),
                "cli_ver" => cli_ver = Some(val.to_string()),
                "port" => port = Some(val.parse::<u16>()?),
                _ => {
                    eprintln!("TURION: Unknown parameter {key} ({val})");
                }
            }
        }

        let username = username.unwrap();
        let password = password.unwrap();
        let port = port.unwrap();

        Ok(Self {
            hostname,
            port,
            password,
            username,
            serial,
            net_ver,
            dev_ver,
            cli_id,
            cli_ver,
        })
    }
}

#[derive(Debug)]
struct NoCertificateVerification(CryptoProvider);

impl NoCertificateVerification {
    pub fn new(provider: CryptoProvider) -> Self {
        Self(provider)
    }
}

impl rustls::client::danger::ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct CameraCmdFrameHeader {
    pub frame_len: u32,
    pub itrack: i32,
    pub flags: i32,
    padding: u32,
}

impl From<[u8; 16]> for CameraCmdFrameHeader {
    fn from(mut value: [u8; 16]) -> Self {
        unsafe {
            let ptr = value.as_mut_ptr() as *mut Self;

            *ptr
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct CameraCmdPacket {
    pub cmd: [u32; 4],
    pub user: [u8; 32],
    pub pass: [u8; 32],
}

impl CameraCmdPacket {
    pub fn new(req_type: i32, user: &str, pass: &str, is_start: bool) -> Self {
        let mut res = CameraCmdPacket {
            cmd: [0x0; 4],
            user: [0x0; 32],
            pass: [0x0; 32],
        };

        res.cmd[1] = req_type as u32;

        if is_start {
            res.cmd[0] |= 0x40;
        }

        let user = user.as_bytes();
        let pass = pass.as_bytes();

        res.user[..user.len()].copy_from_slice(user);
        res.pass[..pass.len()].copy_from_slice(pass);

        res
    }

    pub const fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(self as *const _ as *const u8, core::mem::size_of::<Self>())
        }
    }
}

#[derive(Debug)]
pub struct LocalTunnelConnection {
    poll: mio::Poll,
    socket: TcpStream,
    tls_conn: rustls::ClientConnection,
}

impl io::Write for LocalTunnelConnection {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.tls_conn.writer().write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tls_conn.writer().flush()
    }
}

impl io::Read for LocalTunnelConnection {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.tls_conn.reader().read(bytes)
    }
}

impl LocalTunnelConnection {
    const TOKEN: mio::Token = mio::Token(0);

    fn new(
        sock: TcpStream,
        server_name: String,
        cfg: Arc<rustls::ClientConfig>,
    ) -> io::Result<Self> {
        let mut res = Self {
            poll: mio::Poll::new()?,
            socket: sock,
            tls_conn: rustls::ClientConnection::new(cfg, server_name.try_into().unwrap()).unwrap(),
        };

        let interest = res.event_set();
        res.poll
            .registry()
            .register(&mut res.socket, Self::TOKEN, interest)?;

        Ok(res)
    }

    fn event_set(&self) -> mio::Interest {
        let rd = self.tls_conn.wants_read();
        let wr = self.tls_conn.wants_write();

        if rd && wr {
            mio::Interest::READABLE | mio::Interest::WRITABLE
        } else if wr {
            mio::Interest::WRITABLE
        } else {
            mio::Interest::READABLE
        }
    }

    fn handshake(&mut self) -> io::Result<()> {
        let mut events = mio::Events::with_capacity(8);
        while self.tls_conn.is_handshaking() {
            loop {
                match self.poll.poll(&mut events, None) {
                    Ok(_) => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }

            // Register again
            let interest = self.event_set();
            self.poll
                .registry()
                .reregister(&mut self.socket, Self::TOKEN, interest)?;

            match self.tls_conn.complete_io(&mut self.socket) {
                Ok(_) => continue,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    pub fn process_events<R>(&mut self, can_block: bool, mut read_cb: R) -> io::Result<()>
    where
        R: FnMut(&mut LocalTunnelConnection) -> io::Result<()>,
    {
        let mut events = mio::Events::with_capacity(8);

        let res = loop {
            // Register again
            let interest = self.event_set();
            self.poll
                .registry()
                .reregister(&mut self.socket, Self::TOKEN, interest)?;

            match self.poll.poll(&mut events, Some(Duration::from_nanos(10))) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted && can_block => continue,
                Err(e) => break Err(e),
            }

            match self.tls_conn.complete_io(&mut self.socket) {
                Ok((read_count, write_count)) => break Ok((read_count, write_count)),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => break Err(e),
            }
        };

        res?;

        /* Always assume data as we might have processed something previously and not finish reading */
        read_cb(self)?;

        Ok(())
    }
}

#[derive(Debug)]
enum LocalTunnelState {
    Initial,

    ProcessStream,

    ReceivingSample {
        header: CameraCmdFrameHeader,
        data: Vec<u8>,
        remaining_bytes: usize,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("Local tunnel error: {0}")]
pub struct LocalTunnelError(&'static str);

#[derive(Debug)]
pub struct LocalTunnel {
    pub settings: LocalSettings,
    conn_opt: Option<LocalTunnelConnection>,
    req_type_opt: Option<i32>,
    state_opt: Option<LocalTunnelState>,
    own_sample_buffer: bool,
}

#[derive(Debug)]
#[repr(C)]
pub struct BambuSample {
    pub itrack: i32,
    pub size: i32,
    pub flags: i32,
    pub buffer: *mut u8,
    pub decode_time: u64,
}

impl BambuSample {
    pub fn set_buffer(&mut self, header: CameraCmdFrameHeader, mut data: Vec<u8>) {
        self.itrack = header.itrack;
        self.flags = header.flags;

        /* Safety: We need this to match for destroy_buffer */
        assert!(data.len() == data.capacity());

        self.buffer = data.as_mut_ptr();
        self.size = data.len() as _;

        /* XXX: Figure this out, seems to be monotonic */
        self.decode_time = 0;

        /* Forget the buffer, we will destroy it next round */
        std::mem::forget(data);
    }

    pub fn destroy_buffer(&mut self) {
        if self.size <= 0 {
            return;
        }

        // Reconstruct Vec and drop
        /* Safety: The size and capacity should match as we control them in the reception codepath */
        let _ = unsafe { Vec::from_raw_parts(self.buffer, self.size as _, self.size as _) };

        self.size = 0;
    }
}

impl LocalTunnel {
    pub const fn new(settings: LocalSettings) -> Self {
        Self {
            settings,
            conn_opt: None,
            req_type_opt: None,
            state_opt: None,
            own_sample_buffer: false,
        }
    }

    fn ensure_connected(&self) -> Result<()> {
        if self.conn_opt.is_none() {
            bail!(LocalTunnelError("stream not opened"))
        }

        Ok(())
    }

    pub fn open(&mut self) -> Result<()> {
        if self.conn_opt.is_some() {
            bail!(LocalTunnelError("stream already opened"))
        }

        if self.state_opt.is_some() {
            bail!(LocalTunnelError("stream already opened"))
        }

        let mut cfg =
            rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS12])
                .with_root_certificates(RootCertStore::empty())
                .with_no_client_auth();
        let mut dangerous_config = rustls::ClientConfig::dangerous(&mut cfg);
        dangerous_config.set_certificate_verifier(Arc::new(NoCertificateVerification::new(
            provider::default_provider(),
        )));

        let sock_addr = (self.settings.hostname.as_str(), self.settings.port)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap();
        let sock = TcpStream::connect(sock_addr)?;

        let mut conn =
            LocalTunnelConnection::new(sock, self.settings.hostname.clone(), Arc::new(cfg))?;

        conn.handshake()?;

        self.conn_opt = Some(conn);
        self.state_opt = Some(LocalTunnelState::Initial);

        Ok(())
    }

    pub fn start(&mut self, req_type: i32) -> Result<()> {
        self.ensure_connected()?;

        let conn = self.conn_opt.as_mut().unwrap();

        match self.state_opt {
            None | Some(LocalTunnelState::Initial) => {}
            _ => bail!(LocalTunnelError("stream already started")),
        }

        let packet = CameraCmdPacket::new(
            req_type,
            &self.settings.username,
            &self.settings.password,
            true,
        );

        conn.write_all(packet.as_bytes())?;
        conn.process_events(true, |_| Ok(()))?;

        self.state_opt = Some(LocalTunnelState::ProcessStream);
        self.req_type_opt = Some(req_type);

        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        self.ensure_connected()?;

        let conn = self.conn_opt.as_mut().unwrap();

        match self.state_opt {
            None | Some(LocalTunnelState::Initial) => bail!(LocalTunnelError("stream not started")),
            _ => {}
        }

        let packet = CameraCmdPacket::new(
            self.req_type_opt.unwrap(),
            &self.settings.username,
            &self.settings.password,
            false,
        );

        conn.write_all(packet.as_bytes())?;
        conn.process_events(true, |_| Ok(()))?;

        self.state_opt = Some(LocalTunnelState::Initial);

        Ok(())
    }

    pub fn read_sample(&mut self, sample: &mut BambuSample) -> Result<()> {
        self.ensure_connected()?;

        /* Ensure that we have no undefined state on first read...
         * of course this is highly unsafe but not sure
         * what we can do better here... */
        if !self.own_sample_buffer {
            *sample = BambuSample {
                buffer: core::ptr::null_mut(),
                itrack: 0,
                size: 0,
                flags: 0,
                decode_time: 0,
            };

            self.own_sample_buffer = true;
        }

        sample.destroy_buffer();

        let conn = self.conn_opt.as_mut().unwrap();

        match &mut self.state_opt {
            None | Some(LocalTunnelState::Initial) => bail!(LocalTunnelError("stream not started")),
            Some(LocalTunnelState::ProcessStream) => {
                let mut switch_state = false;

                let mut raw_header = [0x0u8; 16];
                let mut data = Vec::new();

                conn.process_events(false, |conn| {
                    conn.read_exact(&mut raw_header)?;
                    switch_state = true;

                    Ok(())
                })?;

                if switch_state {
                    let header = CameraCmdFrameHeader::from(raw_header);
                    data.reserve(header.frame_len as _);

                    self.state_opt = Some(LocalTunnelState::ReceivingSample {
                        header,
                        remaining_bytes: data.capacity(),
                        data,
                    });
                }

                // We say that we got interrupted to get on the next state
                bail!(Error::new(ErrorKind::Interrupted, "next state (receiving)"))
            }
            Some(LocalTunnelState::ReceivingSample {
                header,
                data,
                remaining_bytes: 0,
            }) => {
                sample.set_buffer(*header, data.clone());
                self.state_opt = Some(LocalTunnelState::ProcessStream);
            }

            Some(LocalTunnelState::ReceivingSample {
                header: _,
                data,
                remaining_bytes,
            }) => {
                conn.process_events(false, |conn| {
                    let mut buffer = [0u8; 4096];

                    while *remaining_bytes != 0 {
                        let bufffer_max_len = (*remaining_bytes).min(buffer.len());

                        let n = conn.read(&mut buffer[..bufffer_max_len])?;

                        if n == 0 {
                            break;
                        }

                        data.extend_from_slice(&buffer[..n]);
                        *remaining_bytes -= n;
                    }

                    /* Should be impossible to get here without some full sample */
                    assert!(*remaining_bytes == 0);

                    Ok(())
                })?;

                // We say that we got interrupted to get on the next state
                bail!(Error::new(ErrorKind::Interrupted, "next state (finishing)"))
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let local_settings =
            LocalSettings::from_url("bambu:///local/127.0.0.1.?port=1234&user=elysia&passwd=ego")
                .unwrap();
        assert_eq!(local_settings.hostname, "127.0.0.1");
        assert_eq!(local_settings.port, 1234);
        assert_eq!(local_settings.username, "elysia");
        assert_eq!(local_settings.password, "ego");
    }
}
