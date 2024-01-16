#![no_std]

use core::marker;
use core::time;

mod command;
mod encoding;
mod error;
mod full_duplex;
mod handler;
mod param;
mod params;
pub mod transport;
pub mod types;

pub use error::Error;
use types::ConnectionState;

const BUFFER_CAPACITY: usize = 4096;

#[derive(Debug)]
pub struct Wifi<T> {
    pub handler: handler::Handler<T>,
    led_init: bool,
}

#[derive(Debug)]
pub struct Client<T> {
    socket: types::Socket,
    buffer_offset: usize,
    buffer: arrayvec::ArrayVec<[u8; BUFFER_CAPACITY]>,
    phantom: marker::PhantomData<T>,
}

#[derive(Debug)]
pub struct ServerUdp<T> {
    socket: types::Socket,
    client_socket: Option<types::Socket>,
    buffer_offset: usize,
    buffer: arrayvec::ArrayVec<[u8; BUFFER_CAPACITY]>,
    phantom: marker::PhantomData<T>,
}

impl<T> Wifi<T>
where
    T: transport::Transport,
{
    pub fn new(transport: T) -> Self {
        let handler = handler::Handler::new(transport);
        let led_init = false;
        Self { handler, led_init }
    }

    pub fn get_firmware_version(
        &mut self,
    ) -> Result<arrayvec::ArrayVec<[u8; 16]>, error::Error<T::Error>> {
        self.handler.get_firmware_version()
    }

    pub fn set_led(&mut self, r: u8, g: u8, b: u8) -> Result<(), error::Error<T::Error>> {
        if !self.led_init {
            self.handler.pin_mode(25, types::PinMode::Output)?;
            self.handler.pin_mode(26, types::PinMode::Output)?;
            self.handler.pin_mode(27, types::PinMode::Output)?;
            self.led_init = true;
        }

        #[cfg(feature = "arduino-nano-connect")]
        let (r, g, b) = (255 - r, 255 - g, 255 - b);

        #[cfg(not(feature = "arduino-nano-connect"))]
        {
            self.handler.analog_write(25, r)?;
            self.handler.analog_write(26, g)?;
            self.handler.analog_write(27, b)?;
        }

        #[cfg(feature = "arduino-nano-connect")]
        {
            self.handler.analog_write(25, g)?;
            self.handler.analog_write(26, b)?;
            self.handler.analog_write(27, r)?;
        }

        Ok(())
    }

    pub fn configure(
        &mut self,
        config: types::Config,
        connect_timeout: Option<time::Duration>,
    ) -> Result<(), error::Error<T::Error>> {
        match config {
            types::Config::Station(station_config) => match station_config.network {
                types::NetworkConfig::Open { ssid } => self.handler.set_network(ssid)?,
                types::NetworkConfig::Password { ssid, password } => {
                    self.handler.set_passphrase(ssid, password)?
                }
            },
            types::Config::AccessPoint(ap_config) => {
                let channel = ap_config.channel.unwrap_or(1);
                match ap_config.network {
                    types::NetworkConfig::Open { ssid } => {
                        self.handler.set_ap_network(ssid, channel)?
                    }
                    types::NetworkConfig::Password { ssid, password } => {
                        self.handler.set_ap_passphrase(ssid, password, channel)?
                    }
                }

                let mut tries = 0;

                while tries < 10 {
                    tries += 1;

                    // TODO: Actual Timeout
                    self.handler.delay(time::Duration::from_millis(100))?;

                    let status = self.handler.get_connection_state()?;

                    if (status != ConnectionState::IdleStatus)
                        && (status != ConnectionState::NoSsidAvail)
                        && (status != ConnectionState::ScanCompleted)
                    {
                        break;
                    }
                }

                let status = self.handler.get_connection_state()?;

                self.handler.delay(time::Duration::from_millis(100))?;

                if status != ConnectionState::ApListening {
                    return Err(error::Error::BadConnectionStatus(
                        num_enum::TryFromPrimitiveError {
                            number: status.into(),
                        },
                    ));
                }
            }
        }

        if let Some(connect_timeout) = connect_timeout {
            self.await_connection_state(types::ConnectionState::Connected, connect_timeout)?;
        }

        Ok(())
    }

    pub fn await_connection_state(
        &mut self,
        connection_state: types::ConnectionState,
        timeout: time::Duration,
    ) -> Result<(), error::Error<T::Error>> {
        const POLL_INTEVAL: time::Duration = time::Duration::from_millis(100);

        let mut total_time = time::Duration::new(0, 0);

        let mut actual_connection_state;
        loop {
            actual_connection_state = self.handler.get_connection_state()?;
            if connection_state == actual_connection_state {
                return Ok(());
            }

            self.handler.delay(POLL_INTEVAL)?;
            // TODO: don't assume the actual SPI transfer takes 0 time :)
            total_time += POLL_INTEVAL;

            if total_time > timeout {
                break;
            }
        }

        Err(error::Error::ConnectionFailure(actual_connection_state))
    }

    pub fn scan_networks(
        &mut self,
    ) -> Result<
        impl Iterator<Item = Result<types::ScannedNetwork, error::Error<T::Error>>> + '_,
        error::Error<T::Error>,
    > {
        self.handler.start_scan_networks()?;
        Ok(self
            .handler
            .get_scanned_networks()?
            .into_iter()
            .enumerate()
            .map(move |(i, ssid)| {
                let i = i as u8;
                let rssi = self.handler.get_scanned_network_rssi(i)?;
                let encryption_type = self.handler.get_scanned_network_encryption_type(i)?;
                let bssid = self.handler.get_scanned_network_bssid(i)?;
                let channel = self.handler.get_scanned_network_channel(i)?;

                Ok(types::ScannedNetwork {
                    ssid,
                    rssi,
                    encryption_type,
                    bssid,
                    channel,
                })
            }))
    }

    pub fn ssid(&mut self) -> Result<arrayvec::ArrayVec<[u8; 32]>, error::Error<T::Error>> {
        self.handler.get_current_ssid()
    }

    pub fn bssid(&mut self) -> Result<arrayvec::ArrayVec<[u8; 6]>, error::Error<T::Error>> {
        self.handler.get_current_bssid()
    }

    pub fn rssi(&mut self) -> Result<i32, error::Error<T::Error>> {
        self.handler.get_current_rssi()
    }

    pub fn encryption_type(&mut self) -> Result<types::EncryptionType, error::Error<T::Error>> {
        self.handler.get_current_encryption_type()
    }

    pub fn resolve(
        &mut self,
        hostname: &str,
    ) -> Result<no_std_net::Ipv4Addr, error::Error<T::Error>> {
        self.handler.request_host_by_name(hostname)?;
        self.handler.get_host_by_name()
    }

    pub fn new_client(&mut self) -> Result<Client<T>, error::Error<T::Error>> {
        let socket = self.handler.get_socket()?;
        let buffer_offset = 0;
        let buffer = arrayvec::ArrayVec::new();
        let phantom = marker::PhantomData;
        Ok(Client {
            socket,
            buffer_offset,
            buffer,
            phantom,
        })
    }
}

impl<T> Client<T>
where
    T: transport::Transport,
{
    pub fn connect_ipv4(
        &mut self,
        wifi: &mut Wifi<T>,
        ip: no_std_net::Ipv4Addr,
        port: u16,
        protocol_mode: types::ProtocolMode,
    ) -> Result<(), error::Error<T::Error>> {
        wifi.handler
            .start_client_by_ip(ip, port, self.socket, protocol_mode)
    }

    pub fn send(
        &mut self,
        wifi: &mut Wifi<T>,
        data: &[u8],
    ) -> Result<usize, error::Error<T::Error>> {
        let len = data.len().min(u16::max_value() as usize);
        let sent = wifi.handler.send_data(self.socket, &data[..len])?;
        wifi.handler.check_data_sent(self.socket)?;
        Ok(sent)
    }

    pub fn send_all(
        &mut self,
        wifi: &mut Wifi<T>,
        mut data: &[u8],
    ) -> Result<(), error::Error<T::Error>> {
        while !data.is_empty() {
            let len = self.send(wifi, data)?;
            data = &data[len..];
        }
        Ok(())
    }

    pub fn state(&mut self, wifi: &mut Wifi<T>) -> Result<types::TcpState, error::Error<T::Error>> {
        wifi.handler.get_client_state(self.socket)
    }

    pub fn recv(
        &mut self,
        wifi: &mut Wifi<T>,
        data: &mut [u8],
    ) -> Result<usize, error::Error<T::Error>> {
        if self.buffer_offset >= self.buffer.len() {
            self.buffer.clear();
            self.buffer
                .try_extend_from_slice(&[0; BUFFER_CAPACITY])
                .unwrap();
            let recv_len = wifi
                .handler
                .get_data_buf(self.socket, self.buffer.as_mut())?;
            self.buffer.truncate(recv_len);
            self.buffer_offset = 0;
            log::debug!("fetched new buffer of len {}", self.buffer.len());
        }

        let len = data.len().min(self.buffer.len() - self.buffer_offset);
        data[..len].copy_from_slice(&self.buffer[self.buffer_offset..self.buffer_offset + len]);
        self.buffer_offset += len;
        Ok(len)
    }

    pub fn recv_exact(
        &mut self,
        wifi: &mut Wifi<T>,
        mut data: &mut [u8],
    ) -> Result<(), error::Error<T::Error>> {
        while !data.is_empty() {
            let len = self.recv(wifi, data)?;
            data = &mut data[len..];
        }
        Ok(())
    }
}

impl<T> ServerUdp<T>
where
    T: transport::Transport,
{
    pub fn start_server(wifi: &mut Wifi<T>, port: u16) -> Result<Self, error::Error<T::Error>> {
        match wifi.handler.start_udp_server(port) {
            Ok(socket) => {
                let buffer_offset = 0;
                let buffer = arrayvec::ArrayVec::new();
                let phantom = marker::PhantomData;
                Ok(ServerUdp {
                    socket,
                    client_socket: None,
                    buffer_offset,
                    buffer,
                    phantom,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub fn start_server_at_ip(
        wifi: &mut Wifi<T>,
        ip: no_std_net::Ipv4Addr,
        port: u16,
    ) -> Result<Self, error::Error<T::Error>> {
        match wifi.handler.start_udp_server_multicast(ip, port) {
            Ok(socket) => {
                let buffer_offset = 0;
                let buffer = arrayvec::ArrayVec::new();
                let phantom = marker::PhantomData;
                Ok(ServerUdp {
                    socket,
                    client_socket: None,
                    buffer_offset,
                    buffer,
                    phantom,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub fn start_packet(
        &mut self,
        wifi: &mut Wifi<T>,
        destination: no_std_net::Ipv4Addr,
        dest_port: u16,
    ) -> Result<(), error::Error<T::Error>> {
        match wifi.handler.begin_udp_packet(destination, dest_port) {
            Ok(client_sock) => {
                self.client_socket = Some(client_sock);
                Ok(())
            }
            Err(_) => {
                log::error!("failed to start packet");
                Err(error::Error::SendDataUdp("381"))
            }
        }
    }

    pub fn write_data(
        &mut self,
        wifi: &mut Wifi<T>,
        data: &[u8],
    ) -> Result<(), error::Error<T::Error>> {
        wifi.handler.udp_write(self.socket, data)
    }

    pub fn end_packet(&mut self, wifi: &mut Wifi<T>) -> Result<(), error::Error<T::Error>> {
        match self.client_socket {
            Some(sock) => wifi.handler.end_udp_packet(sock),
            None => {
                log::error!("no client socket");
                Err(error::Error::SendDataUdp("399"))
            }
        }
    }

    pub fn read_packet(
        &mut self,
        wifi: &mut Wifi<T>,
        buffer: &mut [u8],
    ) -> Result<usize, error::Error<T::Error>> {
        if self.buffer_offset >= self.buffer.len() {
            self.buffer.clear();
            match self.buffer.try_extend_from_slice(&[0; BUFFER_CAPACITY]) {
                Ok(_) => {}
                Err(_) => {
                    log::error!("buffer overflow");
                    return Err(error::Error::DataTooLong);
                }
            }
            let recv_len = wifi
                .handler
                .get_data_buf(self.socket, self.buffer.as_mut())?;
            self.buffer.truncate(recv_len);
            self.buffer_offset = 0;
            log::debug!("fetched new buffer of len {}", self.buffer.len());
        }

        let len = buffer.len().min(self.buffer.len() - self.buffer_offset);
        buffer[..len].copy_from_slice(&self.buffer[self.buffer_offset..self.buffer_offset + len]);
        self.buffer_offset += len;
        Ok(len)
    }

    pub fn stop_server(self, wifi: &mut Wifi<T>) -> Result<(), error::Error<T::Error>> {
        wifi.handler.stop_client(self.socket)
    }

    pub fn get_socket(&self) -> types::Socket {
        self.socket
    }
}
