//! Actor managing a particular device.

use actix::{Actor, Context, Addr};
use slog::{Logger, info, o};

use crate::model::*;

pub struct DeviceManager {
    id: DeviceId,
    logger: Logger,

    state: DeviceState
}

impl DeviceManager {
    /// Creates a new `DeviceManager` for the given device ID. The device manager will not update
    /// its state until it is started in Actix.
    pub fn new(id: DeviceId, logger: &Logger) -> DeviceManager {
        DeviceManager {
            id: id.clone(),
            logger: logger.new(o!("device" => id)),
            state: DeviceState::Unknown
        }
    }
}

impl Actor for DeviceManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!(&self.logger, "Starting device manager");
    }
}