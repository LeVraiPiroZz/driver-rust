use std::io::{Read, Write};
use std::net::TcpStream;

/// TcpStream wrapper with internal, utility functions
pub(super) struct ElevatorSocket {
    sock: TcpStream
}

impl ElevatorSocket {
    pub(super) fn new(sock: TcpStream) -> Self {
        Self { sock }
    }

    /// Send elevator direction command to the elevator over TCP
    pub(super) fn motor_direction(&mut self, dirn: u8) {
        let buf = [1, dirn, 0, 0];
        self.sock.write(&buf).unwrap();
    }

    /// Send call button light command to the elevator over TCP
    pub(super) fn call_button_light(&mut self, floor: u8, call: u8, on: bool) {
        let buf = [2, call, floor, on as u8];
        self.sock.write(&buf).unwrap();
    }

    /// Send light indicator command to the elevator over TCP
    pub(super) fn floor_indicator(&mut self, floor: u8) {
        let buf = [3, floor, 0, 0];
        self.sock.write(&buf).unwrap();
    }

    /// Send door light command to the elevator over TCP
    pub(super) fn door_light(&mut self, on: bool) {
        let buf = [4, on as u8, 0, 0];
        self.sock.write(&buf).unwrap();
    }

    /// Send stop (emergency) light command to the elevator over TCP
    pub(super) fn stop_button_light(&mut self, on: bool) {
        let buf = [5, on as u8, 0, 0];
        self.sock.write(&buf).unwrap();
    }

    /// Query the state of a specific call button and return it's state (true is pressed)
    pub(super) fn call_button(&mut self, floor: u8, call: u8) -> bool {
        let mut buf = [6, call, floor, 0];
        self.sock.write(&mut buf).unwrap();
        self.sock.read(&mut buf).unwrap();
        buf[1] != 0
    }

    /// Query the current floor of the cabin, can be None if the cabin is in between floor
    pub(super) fn floor_sensor(&mut self) -> Option<u8> {
        let mut buf = [7, 0, 0, 0];
        self.sock.write(&buf).unwrap();
        self.sock.read(&mut buf).unwrap();
        if buf[1] != 0 {
            Some(buf[2])
        } else {
            None
        }
    }

    /// Query the state of the stop (emergency) button and return it's state (true is pressed)
    pub(super) fn stop_button(&mut self) -> bool {
        let mut buf = [8, 0, 0, 0];
        self.sock.write(&buf).unwrap();
        self.sock.read(&mut buf).unwrap();
        buf[1] != 0
    }

    /// Query the state of door obstruction and return it's state (true is door being obstructed)
    pub(super) fn obstruction(&mut self) -> bool {
        let mut buf = [9, 0, 0, 0];
        self.sock.write(&buf).unwrap();
        self.sock.read(&mut buf).unwrap();
        buf[1] != 0
    }
}


