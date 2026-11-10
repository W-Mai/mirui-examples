// Array characteristic derives emit redundant borrows in generated code.
#![allow(clippy::needless_borrows_for_generic_args)]

use trouble_host::prelude::*;

#[gatt_service(uuid = "7e4b0001-8c20-4b6d-a52b-9f7021cf4420")]
pub(super) struct Prompter {
    #[characteristic(uuid = "7e4b0002-8c20-4b6d-a52b-9f7021cf4420", read, write)]
    pub(super) control: [u8; 20],
    #[characteristic(uuid = "7e4b0003-8c20-4b6d-a52b-9f7021cf4420", read)]
    pub(super) status: [u8; 20],
    #[characteristic(uuid = "7e4b0004-8c20-4b6d-a52b-9f7021cf4420", write)]
    pub(super) upload: [u8; 20],
}

#[gatt_server]
pub(super) struct Server {
    pub(super) prompter: Prompter,
}
