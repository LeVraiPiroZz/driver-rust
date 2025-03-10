use std::convert::TryInto;
use std::time::*;

use crossbeam_channel as cbc;

use driver_rust::elevio::elev as e;
use driver_rust::elevio::elev::{ElevatorEvent, MotorDirection};

const ELEV_NUM_FLOORS: u8 = 4;
const LAST_FLOOR: u8 = ELEV_NUM_FLOORS - 1;

fn main() -> std::io::Result<()> {
    // Initialize and connect to the elevator.

    let mut elevator = e::Elevator::init("localhost:15657", ELEV_NUM_FLOORS)?;
    println!("Elevator started:\n{:#?}", elevator);

    // Sets a poll period, this should be a small period or the events will be delayed or lost.
    let poll_period = Duration::from_millis(25);
    elevator.event_loop(poll_period);

    let mut direction = MotorDirection::Down;

    loop {
        cbc::select! {
            // Receive events from the elevator
            recv(elevator.event_receiver) -> event => {
                let event = event.unwrap();
                println!("{event}");
                // Pattern matching over the event type, and triggers relevant action.
                match event{
                    ElevatorEvent::CallButton{ floor, call } => elevator.call_button_light(floor, call, true),
                    ElevatorEvent::FloorSensor{ floor } => {
                        direction = match floor {
                            Some(0) => MotorDirection::Up,
                            Some(LAST_FLOOR) => MotorDirection::Down,
                            _ => direction
                        };
                        elevator.motor_direction(direction);
                    }
                    ElevatorEvent::Obstruction{ obstructed } => elevator.motor_direction(if obstructed { MotorDirection::Stop } else { direction }),
                    ElevatorEvent::StopButton{ stopped } => {
                        if stopped {
                            for f in 0..ELEV_NUM_FLOORS {
                                for c in 0..3 {
                                    elevator.call_button_light(f, c.try_into().unwrap(), false);
                                }
                            }
                        }
                    }
                }
            },
        }
    }
}
