//! A dummy I2C client

use core::cell::Cell;
use kernel::hil;
use kernel::hil::i2c::I2CMaster;
use sam4l::i2c;

// ===========================================
// Scan for I2C Slaves
// ===========================================

struct ScanClient {
    dev_id: Cell<u8>,
}

static mut SCAN_CLIENT: ScanClient = ScanClient {
    dev_id: Cell::new(1),
};

impl hil::i2c::I2CHwMasterClient for ScanClient {
    fn command_complete(&self, buffer: &'static mut [u8], error: hil::i2c::Error) {
        let mut dev_id = self.dev_id.get();

        if error == hil::i2c::Error::CommandComplete {
            println!("{:#x}", dev_id);
        }

        let dev: &mut I2CMaster = unsafe { &mut i2c::I2C2 };
        if dev_id < 0x7F {
            dev_id += 1;
            self.dev_id.set(dev_id);
            dev.write(dev_id, buffer, 2);
        } else {
            println!(
                "Done scanning for I2C devices. Buffer len: {}",
                buffer.len()
            );
        }
    }
}

pub fn i2c_scan_slaves() {
    static mut DATA: [u8; 255] = [0; 255];

    let dev = unsafe { &mut i2c::I2C2 };

    let i2c_client = unsafe { &SCAN_CLIENT };
    dev.set_master_client(i2c_client);

    let dev: &mut I2CMaster = dev;
    dev.enable();

    println!("Scanning for I2C devices...");
    dev.write(i2c_client.dev_id.get(), unsafe { &mut DATA }, 2);
}

// ===========================================
// Test FXOS8700CQ
// ===========================================

#[derive(Copy, Clone)]
enum AccelClientState {
    ReadingWhoami,
    Activating,
    Deactivating,
    ReadingAccelData,
}

struct AccelClient {
    state: Cell<AccelClientState>,
}

static mut ACCEL_CLIENT: AccelClient = AccelClient {
    state: Cell::new(AccelClientState::ReadingWhoami),
};

impl hil::i2c::I2CHwMasterClient for AccelClient {
    fn command_complete(&self, buffer: &'static mut [u8], error: hil::i2c::Error) {
        use self::AccelClientState::*;

        let dev = unsafe { &mut i2c::I2C2 };

        match self.state.get() {
            ReadingWhoami => {
                println!("WHOAMI Register 0x{:x} ({})", buffer[0], error);
                println!("Activating Sensor...");
                buffer[0] = 0x2A as u8; // CTRL_REG1
                buffer[1] = 1; // Bit 1 sets `active`
                dev.write(0x1e, i2c::START | i2c::STOP, buffer, 2);
                self.state.set(Activating);
            }
            Activating => {
                println!("Sensor Activated ({})", error);
                buffer[0] = 0x01 as u8; // X-MSB register
                                        // Reading 6 bytes will increment the register pointer through
                                        // X-MSB, X-LSB, Y-MSB, Y-LSB, Z-MSB, Z-LSB
                dev.write_read(0x1e, buffer, 1, 6);
                self.state.set(ReadingAccelData);
            }
            ReadingAccelData => {
                let x = (((buffer[0] as u16) << 8) | buffer[1] as u16) as usize;
                let y = (((buffer[2] as u16) << 8) | buffer[3] as u16) as usize;
                let z = (((buffer[4] as u16) << 8) | buffer[5] as u16) as usize;

                let x = ((x >> 2) * 976) / 1000;
                let y = ((y >> 2) * 976) / 1000;
                let z = ((z >> 2) * 976) / 1000;

                println!(
                    "Accel data ready x: {}, y: {}, z: {} ({})",
                    x >> 2,
                    y >> 2,
                    z >> 2,
                    error
                );

                buffer[0] = 0x01 as u8; // X-MSB register
                                        // Reading 6 bytes will increment the register pointer through
                                        // X-MSB, X-LSB, Y-MSB, Y-LSB, Z-MSB, Z-LSB
                dev.write_read(0x1e, buffer, 1, 6);
                self.state.set(ReadingAccelData);
            }
            Deactivating => {
                println!("Sensor deactivated ({})", error);
                println!("Reading Accel's WHOAMI...");
                buffer[0] = 0x0D as u8; // 0x0D == WHOAMI register
                dev.write_read(0x1e, buffer, 1, 1);
                self.state.set(AccelClientState::ReadingWhoami);
            }
        }
    }
}

pub fn i2c_accel_test() {
    static mut DATA: [u8; 255] = [0; 255];

    let dev = unsafe { &mut i2c::I2C2 };

    let i2c_client = unsafe { &ACCEL_CLIENT };
    dev.set_master_client(i2c_client);
    dev.enable();

    let buf = unsafe { &mut DATA };
    println!("Reading Accel's WHOAMI...");
    buf[0] = 0x0D as u8; // 0x0D == WHOAMI register
    dev.write_read(0x1e, buf, 1, 1);
    i2c_client.state.set(AccelClientState::ReadingWhoami);
}

// ===========================================
// Test LI
// ===========================================

#[derive(Copy, Clone)]
enum LiClientState {
    Enabling,
    ReadingLI,
}

struct LiClient {
    state: Cell<LiClientState>,
}

static mut LI_CLIENT: LiClient = LiClient {
    state: Cell::new(LiClientState::Enabling),
};

impl hil::i2c::I2CHwMasterClient for LiClient {
    fn command_complete(&self, buffer: &'static mut [u8], error: hil::i2c::Error) {
        use self::LiClientState::*;

        let dev = unsafe { &mut i2c::I2C2 };

        match self.state.get() {
            Enabling => {
                println!("Reading Lumminance Registers ({})", error);
                buffer[0] = 0x02 as u8;
                buffer[0] = 0;
                dev.write_read(0x44, buffer, 1, 2);
                self.state.set(ReadingLI);
            }
            ReadingLI => {
                let intensity = ((buffer[1] as usize) << 8) | buffer[0] as usize;
                println!("Light Intensity: {}% ({})", (intensity * 100) >> 16, error);
                buffer[0] = 0x02 as u8;
                dev.write_read(0x44, buffer, 1, 2);
                self.state.set(ReadingLI);
            }
        }
    }
}

pub fn i2c_li_test() {
    static mut DATA: [u8; 255] = [0; 255];

    unsafe {
        use sam4l;
        sam4l::gpio::PA[16].enable_output();
        sam4l::gpio::PA[16].set();
    }

    let dev = unsafe { &mut i2c::I2C2 };

    let i2c_client = unsafe { &LI_CLIENT };
    dev.set_master_client(i2c_client);
    dev.enable();

    let buf = unsafe { &mut DATA };
    println!("Enabling LI...");
    buf[0] = 0;
    buf[1] = 0b10100000;
    buf[2] = 0b00000000;
    dev.write(0x44, i2c::START | i2c::STOP, buf, 3);
    i2c_client.state.set(LiClientState::Enabling);
}
