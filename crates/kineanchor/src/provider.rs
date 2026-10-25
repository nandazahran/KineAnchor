// SPDX-License-Identifier: GPL-3.0-or-later

use kineanchor_core::MotionSample;
use std::sync::{Arc, atomic::AtomicBool, mpsc::Sender};

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderEvent {
    Sample(MotionSample),
    Connected(String),
    Disconnected(String),
    Paired { device: String, token: String },
}

pub trait Provider: Send {
    fn name(&self) -> &'static str;
    /// Blocking loop on its own thread; returns when `stop` is set.
    fn run(self: Box<Self>, tx: Sender<ProviderEvent>, stop: Arc<AtomicBool>);
}
