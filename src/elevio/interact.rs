use crate::elevio::elev::ElevatorEvent::{CallButton, FloorSensor, Obstruction, StopButton};
use crate::elevio::elev::{Elevator, ElevatorEvent, ElevatorMessage};
use crate::elevio::sock::ElevatorSocket;
use crossbeam_channel::{select, tick, Receiver, Sender};
use std::convert::TryInto;
use std::thread::spawn;
use std::time;

/// Stores the last sent value for every possible events.
/// This is used internally to prevent event *flooding* that could take CPU time.
struct ElevatorCurrentState {
    call_buttons: Vec<[bool; 3]>,
    floor_sensor: u8,
    stop_button: bool,
    obstruction: bool,
}

impl ElevatorCurrentState {
    /// Generate the default state of the elevator.
    fn new(num_floors: u8) -> Self {
        Self {
            call_buttons: vec![[false; 3]; num_floors as usize],
            floor_sensor: u8::MAX,
            stop_button: false,
            obstruction: false,
        }
    }
}

/// Handle the event/message loop
struct ElevatorInteraction {
    /// Actual socket of the elevator.
    sock: ElevatorSocket,
    /// Moved event sender from the elevator. Used to dispatch event to the user
    event_sender: Sender<ElevatorEvent>,
    /// Moved message receiver from the elevator. Used to dispatch message to the elevator
    message_receiver: Receiver<ElevatorMessage>,
    /// Number of floors this elevator has.
    num_floors: u8,
    /// Store the current status of the elevator. Avoids event flooding on the user end.
    current_state: ElevatorCurrentState,
}

impl Elevator {
    /// Move relevant element from the elevator and start the event/message dispatch loop.
    /// This can only be used once per [Elevator] instance, using it again will result in panic.
    pub fn event_loop(&mut self, period: time::Duration) {
        let mut e_interact = ElevatorInteraction {
            sock: ElevatorSocket::new(
                self.sock
                    .take()
                    .expect("Cannot launch more than one event_loop on an elevator hardware."),
            ),
            event_sender: self
                .event_sender
                .take()
                .expect("Cannot launch more than one event_loop on an elevator hardware."),
            message_receiver: self
                .message_receiver
                .take()
                .expect("Cannot launch more than one event_loop on an elevator hardware."),
            num_floors: self.num_floors,
            current_state: ElevatorCurrentState::new(self.num_floors),
        };

        spawn(move || {
            // Initialize polling periods.
            let poll_call = tick(period);
            let poll_floor = tick(period);
            let poll_stop = tick(period);
            let poll_obst = tick(period);

            loop {
                select! {
                    // Handle message and dispatch them.
                    recv(e_interact.message_receiver) -> message => {
                        match message.unwrap() {
                            ElevatorMessage::MotorDirection{ direction } => e_interact.sock.motor_direction(direction as u8),
                            ElevatorMessage::CallButtonLight{ floor, call, on } => e_interact.sock.call_button_light(floor, call as u8, on),
                            ElevatorMessage::FloorIndicatorLight{ floor } => e_interact.sock.floor_indicator(floor),
                            ElevatorMessage::DoorOpenLight{ on } => e_interact.sock.door_light(on),
                            ElevatorMessage::StopButtonLight{ on } => e_interact.sock.stop_button_light(on)
                        }
                    }

                    // Handle polling of event.
                    recv(poll_call) -> _ => e_interact.poll_call_buttons_state(),
                    recv(poll_floor) -> _ => e_interact.poll_floor_sensor_state(),
                    recv(poll_stop) -> _ => e_interact.poll_stop_button_state(),
                    recv(poll_obst) -> _ => e_interact.pool_obstruction_sensor_state(),
                }
            }
        });
    }
}

impl ElevatorInteraction {
    /// Used to poll the state of every call button.
    fn poll_call_buttons_state(&mut self) {
        for f in 0..self.num_floors {
            for c in 0..3 {
                let v = self.sock.call_button(f, c);
                if v && self.current_state.call_buttons[f as usize][c as usize] != v {
                    self.event_sender
                        .send(CallButton {
                            floor: f,
                            call: c.try_into().unwrap(),
                        })
                        .unwrap();
                }
                self.current_state.call_buttons[f as usize][c as usize] = v;
            }
        }
    }

    /// Used to poll the current floor.
    /// Current floor can be None if in-between floor, but this is never transmitted.
    fn poll_floor_sensor_state(&mut self) {
        if let Some(f) = self.sock.floor_sensor() {
            if f != self.current_state.floor_sensor {
                self.event_sender.send(FloorSensor { floor: f }).unwrap();
                self.current_state.floor_sensor = f;
            }
        } else {
            self.current_state.floor_sensor = u8::MAX
        }
    }

    /// Used to poll the stop button state.
    fn poll_stop_button_state(&mut self) {
        let v = self.sock.stop_button();
        if self.current_state.stop_button != v {
            self.event_sender.send(StopButton { stopped: v }).unwrap();
            self.current_state.stop_button = v;
        }
    }

    /// Used to poll the door obstruction state.
    fn pool_obstruction_sensor_state(&mut self) {
        let v = self.sock.obstruction();
        if self.current_state.obstruction != v {
            self.event_sender
                .send(Obstruction { obstructed: v })
                .unwrap();
            self.current_state.obstruction = v;
        }
    }
}
