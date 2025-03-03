use crate::elevio::elev::CallType::{Cab, HallDown, HallUp};
use crate::elevio::elev::TcpRemains::{Address, Stream};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::convert::TryFrom;
use std::fmt;
use std::io::*;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};

/// Internal struct used to store and get the address of the elevator hardware (or simulator)
/// This keep access to the address even when the event loop has moved the stream to its own thread.
#[derive(Debug)]
pub(super) enum TcpRemains {
    Stream { sock: TcpStream },
    Address { addr: SocketAddr },
}

impl TcpRemains {
    /// Replace the current TcpStream by its peer address and returns it.
    /// If the value is already an address (i.g second call to take), do nothing and return None instead.
    pub(super) fn take(&mut self) -> Option<TcpStream> {
        let addr = match self {
            Stream { sock } => sock.peer_addr().unwrap(),
            Address { .. } => return None,
        };

        let Stream { sock } = std::mem::replace(self, Address { addr }) else {
            unreachable!("This never happens")
        };
        Some(sock)
    }

    /// Get the peer address of the elevator hardware (or simulator)
    ///
    /// This works even after the stream has been moved.
    fn addr(&self) -> Result<SocketAddr> {
        match self {
            Stream { sock } => sock.peer_addr(),
            Address { addr } => Ok(addr.clone()),
        }
    }
}

/// Enum representing an event.
pub enum ElevatorEvent {
    /// Event received when a user presses a call/order button on the panel.
    /// If the call is a **cab** order, floor represent the destination wanted by the user,
    /// else it's a **hall** order and floor represent the floor where the user pressed the button.
    CallButton { floor: u8, call: CallType },
    /// Event received when the elevator reach a floor
    FloorSensor { floor: u8 },
    /// Event received when the elevator door are stuck open.
    Obstruction { obstructed: bool },
    /// Event received in case of emergency stop pressed by a user.
    StopButton { stopped: bool },
}

impl fmt::Display for ElevatorEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElevatorEvent::CallButton { call, floor } => {
                write!(f, "Call button {call:?}, floor {floor}")
            }
            ElevatorEvent::FloorSensor { floor } => write!(f, "Floor: {floor:#?}"),
            ElevatorEvent::Obstruction { obstructed } => write!(f, "Obstruction: {obstructed:#?}"),
            ElevatorEvent::StopButton { stopped } => write!(f, "Stop button: {stopped:#?}"),
        }
    }
}

/// Enum representing a message. A message is used to interact with the elevator
#[derive(Debug)]
pub enum ElevatorMessage {
    /// Message used to set the direction of the elevator
    MotorDirection { direction: MotorDirection },
    /// Message used to control a call button light of the elevator
    CallButtonLight { on: bool, floor: u8, call: CallType },
    /// Message used to control the floor indicator light
    /// (You can see that as the numeric panel showing the current floor in a real elevator)
    FloorIndicatorLight { floor: u8 },
    /// Message used to control the door light of the elevator
    /// (This is a representation of the actual door since the elevator hardware doesn't have one)
    DoorOpenLight { on: bool },
    /// Message used to control the stop (emergency) button light of the elevator
    StopButtonLight { on: bool },
}

/// Enum representing type of call button pressed
#[derive(Debug, Copy, Clone)]
pub enum CallType {
    /// Hall call button with direction up
    HallUp = 0,
    /// Hall call button with direction down
    HallDown = 1,
    /// Cab call button
    Cab = 2,
}

impl TryFrom<u8> for CallType {
    type Error = ();

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(HallUp),
            1 => Ok(HallDown),
            2 => Ok(Cab),
            _ => Err(()),
        }
    }
}

/// Enum representing the direction of the elevator
/// Those are self-explanatory.
#[derive(Copy, Clone, Debug)]
pub enum MotorDirection {
    Down = u8::MAX as isize,
    Stop = 0,
    Up = 1,
}

/// Entrypoint to interact with the elevator hardware (or simulator)
/// This struct provides every utility methods and attributes to control the elevator.
#[derive(Debug)]
pub struct Elevator {
    /// TcpStream / TcpAddress storage.
    pub(super) sock: TcpRemains,
    /// Sending end of the event channel, used internally to dispatch events to the user
    /// when something has changed on the elevator hardware.
    pub(super) event_sender: Option<Sender<ElevatorEvent>>,
    /// Channel used to receive every event the elevator is sending (i.g floor sensor, call button presses, ...)
    /// This is the entry point to reacting to every relevant event. You **should** use this receiver.
    pub event_receiver: Receiver<ElevatorEvent>,
    /// Sending end of the message channel. Used internally to dispatch message sent by the user.
    message_sender: Sender<ElevatorMessage>,
    /// Receiving end of the channel used internally to dispatch messages to the elevator hardware.
    pub(super) message_receiver: Option<Receiver<ElevatorMessage>>,

    /// Number of floor this elevator is servicing.
    pub num_floors: u8,
}

impl Elevator {
    /// Initializes the elevator struct to be a `num_floors` floors elevator and
    /// connect to the provided address.
    pub fn init<A: ToSocketAddrs>(addr: A, num_floors: u8) -> Result<Elevator> {
        let (event_sender, event_receiver) = unbounded();
        let (message_sender, message_receiver) = unbounded();

        Ok(Self {
            sock: Stream {
                sock: TcpStream::connect(addr)?,
            },
            event_sender: Some(event_sender),
            event_receiver,
            message_sender,
            message_receiver: Some(message_receiver),
            num_floors,
        })
    }

    /// Set the direction of the elevator
    pub fn motor_direction(&mut self, direction: MotorDirection) {
        self.message_sender
            .send(ElevatorMessage::MotorDirection { direction })
            .unwrap()
    }

    /// Set the state of a specific call button light
    pub fn call_button_light(&mut self, floor: u8, call: CallType, on: bool) {
        self.message_sender
            .send(ElevatorMessage::CallButtonLight { floor, call, on })
            .unwrap()
    }

    /// Light up the floor indicator at `floor`, the previous indicator is turned off.
    pub fn floor_indicator(&mut self, floor: u8) {
        self.message_sender
            .send(ElevatorMessage::FloorIndicatorLight { floor })
            .unwrap()
    }

    /// Set the state of the door light. If `on` is [true], the door is opened.
    pub fn door_light(&mut self, on: bool) {
        self.message_sender
            .send(ElevatorMessage::DoorOpenLight { on })
            .unwrap()
    }

    /// Set the state of the stop (emergency) light. If `on` is [true], the elevator is in an emergency state.
    pub fn stop_button_light(&mut self, on: bool) {
        self.message_sender
            .send(ElevatorMessage::StopButtonLight { on })
            .unwrap()
    }
}

impl fmt::Display for Elevator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let addr = self.sock.addr().unwrap();
        write!(f, "Elevator@{}({})", addr, self.num_floors)
    }
}
