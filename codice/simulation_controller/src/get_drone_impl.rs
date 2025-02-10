#![allow(clippy::implicit_hasher)]
use bagel_bomber::BagelBomber;
use crossbeam_channel::{Receiver, Sender};
use getdroned::GetDroned;
use lockheedrustin_drone::LockheedRustin;
use null_pointer_drone::MyDrone as NullPointerDrone;
use rust_do_it::RustDoIt;
use rust_roveri::RustRoveri;
use rustafarian_drone::RustafarianDrone;
use rustastic_drone::RustasticDrone;
use rusty_drones::RustyDrone;

use std::collections::HashMap;

use krusty_drone::KrustyCrapDrone;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

#[must_use]
/// # Panics
pub fn get_drone_impl(
    index: u8,
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    pdr: f32,
) -> Box<dyn Drone + Send> {
    let index = index % 10 + 1;

    match index {
        1 => Box::new(BagelBomber::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        2 => Box::new(LockheedRustin::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        3 => Box::new(NullPointerDrone::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        4 => Box::new(RustafarianDrone::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        5 => Box::new(RustasticDrone::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        6 => Box::new(GetDroned::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        7 => Box::new(RustyDrone::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        8 => Box::new(RustDoIt::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        9 => Box::new(RustRoveri::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        10 => Box::new(KrustyCrapDrone::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )),
        _ => panic!("id should always be in range 1-10"),
    }
}
