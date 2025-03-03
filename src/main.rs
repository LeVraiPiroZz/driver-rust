use std::convert::TryInto;
use std::time::*;

use crossbeam_channel as cbc;

use driver_rust::elevio::elev as e;
use driver_rust::elevio::elev::{ElevatorEvent, MotorDirection};

fn main() -> std::io::Result<()> {
    // Initialize and connect to the elevator.
    let elev_num_floors = 4;
    let mut elevator = e::Elevator::init("localhost:15657", elev_num_floors)?;
    println!("Elevator started:\n{:#?}", elevator);

    // Sets a poll period, this should be a small period or the events will be delayed or lost.
    let poll_period = Duration::from_millis(25);
    elevator.event_loop(poll_period);

    let mut dirn = MotorDirection::Down;
    let one_time_init = cbc::after(Duration::from_millis(100));

    loop {
        cbc::select! {
            // One time event to replace the elevator in case of in-between state.
            recv(one_time_init) -> _ => elevator.motor_direction(dirn),

            // Receive events from the elevator
            recv(elevator.event_receiver) -> event => {
                let event = event.unwrap();
                println!("{event}");
                // Pattern matching over the event type, and triggers relevant action.
                match event{
                    ElevatorEvent::CallButton{ floor, call } => elevator.call_button_light(floor, call, true),
                    ElevatorEvent::FloorSensor{ floor } => {
                        dirn = if floor == 0 {
                            MotorDirection::Up
                        } else if floor == elev_num_floors-1 {
                            MotorDirection::Down
                        } else {
                            dirn
                        };
                        elevator.motor_direction(dirn);
                    }
                    ElevatorEvent::Obstruction{ obstructed } => elevator.motor_direction(if obstructed { MotorDirection::Stop } else { dirn }),
                    ElevatorEvent::StopButton{ stopped } => {
                        if (stopped) {
                            for f in 0..elev_num_floors {
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
