mod board_fr;
mod calc_worker;

use std::collections::VecDeque;
use std::ops::Deref;

use dioxus::prelude::*;
use gloo::timers::callback::Timeout;
use gloo::timers::future::TimeoutFuture;
use js_sys::Uint8Array;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::{
    DedicatedWorkerGlobalScope, MessageEvent, Worker, WorkerGlobalScope, WorkerOptions, WorkerType,
};

use board_fr::QuoridorBoard;
pub use calc_worker::start_webworker;
use calc_worker::*;
use quoridor::{Move, PawnMove};

fn main() {
    wasm_logger::init(wasm_logger::Config::default());

    dioxus_web::launch(app);
}

fn app(cx: Scope) -> Element {
    render! {
        rsx! {
            div {
       QuoridorBoard  {}
            }}
    }
}
