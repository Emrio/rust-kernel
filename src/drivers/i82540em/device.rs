use x86_64::{PhysAddr, instructions::hlt, structures::paging::OffsetPageTable};

use crate::{
    drivers::i82540em::{
        I8254_CTRL_ASDE, I8254_CTRL_RESET, I8254_CTRL_SLU, I8254_EERD_DONE, I8254_REG_CTRL,
        I8254_REG_EERD, I8254_REG_RAH, I8254_REG_RAL,
        tx::{
            CMD_EOP, CMD_IFCS, CMD_RS, REG_TDH, REG_TDT, STA_DD, TX_BUFFERS, TX_DESCS, TX_SIZE,
            TxDescriptor,
        },
    },
    memory::MemoryMapper,
    net::{device::NetworkDevice, ethernet::address::EthernetAddress},
};

type RxHandler = dyn Fn(&[u8]);

pub struct Device<'a> {
    base_address: *mut u32,
    pub(super) mapper: &'a OffsetPageTable<'static>,
    hardware_address: EthernetAddress,
    rx_handler: Option<&'static RxHandler>,
}

impl<'a> Device<'a> {
    pub(super) fn from(mapper: &'a OffsetPageTable<'static>, bar0: u32) -> Self {
        let base_address = mapper.get_virt_mut(PhysAddr::new((bar0 & 0xfffffff8u32) as u64));

        Self {
            base_address,
            mapper,
            hardware_address: EthernetAddress::BROADCAST,
            rx_handler: None,
        }
    }

    pub(super) fn write_register(&self, register: usize, value: u32) {
        unsafe { self.base_address.byte_add(register).write_volatile(value) }
    }

    pub(super) fn read_register(&self, register: usize) -> u32 {
        unsafe { self.base_address.byte_add(register).read_volatile() }
    }

    fn read_eeprom(&self, address: u8) -> u16 {
        // TODO: lock with EECD before reading?

        let packet = (address as u32) << 8 | 1;
        self.write_register(I8254_REG_EERD, packet);

        loop {
            let result = self.read_register(I8254_REG_EERD);

            if result & I8254_EERD_DONE != 0 {
                return (result >> 16) as u16;
            }

            hlt();
        }
    }

    pub(super) fn reset_nic_and_fetch_hw_address(&mut self) {
        self.hardware_address = self.reset_nic();
    }

    pub(super) fn reset_nic(&self) -> EthernetAddress {
        let mut device_control = self.read_register(I8254_REG_CTRL);
        device_control |= I8254_CTRL_RESET;
        self.write_register(I8254_REG_CTRL, device_control);

        while self.read_register(I8254_REG_CTRL) & I8254_CTRL_RESET != 0 {
            hlt();
        }

        let mut device_control = self.read_register(I8254_REG_CTRL);
        device_control |= I8254_CTRL_ASDE | I8254_CTRL_SLU;
        self.write_register(I8254_REG_CTRL, device_control);

        let b0 = self.read_eeprom(0);
        let b1 = self.read_eeprom(1);
        let b2 = self.read_eeprom(2);

        let hwaddr = EthernetAddress::from_u16(b0, b1, b2);
        kprintln!("{hwaddr}");

        self.write_register(I8254_REG_RAL, (b1 as u32) << 16 | (b0 as u32));
        self.write_register(I8254_REG_RAH, b2 as u32 | /* Address valid */ (1 << 31));

        hwaddr
    }

    pub fn rx_handler(&self) -> Option<&'static RxHandler> {
        self.rx_handler
    }
}

impl<'a> NetworkDevice for Device<'a> {
    fn send_packet(&self, buffer: &[u8]) {
        let tail = self.read_register(REG_TDT) as usize;
        let head = self.read_register(REG_TDH) as usize;

        if (tail + 1) % TX_SIZE == head {
            kprintln!("Cannot send packet: buffer full");
        }

        unsafe {
            TX_BUFFERS[tail][0..buffer.len()].copy_from_slice(buffer);

            let descriptor = &raw mut TX_DESCS[tail];
            descriptor.write_volatile(TxDescriptor {
                buffer_address: self.mapper.get_physical(&raw const TX_BUFFERS[tail]),
                length: buffer.len() as u16,
                checksum_offset: 0,
                command: CMD_EOP | CMD_IFCS | CMD_RS,
                status: 0,
                checksum_start: 0,
                special: 0,
            });
        }

        self.write_register(REG_TDT, ((tail + 1) % TX_SIZE) as u32);

        let status = unsafe { &raw const TX_DESCS[tail].status };

        while unsafe { status.read_volatile() } & STA_DD == 0 {
            hlt();
        }
    }

    fn hardware_address(&self) -> EthernetAddress {
        self.hardware_address
    }

    fn setup_device_rx(&mut self, handler_fn: &'static dyn Fn(&[u8])) {
        self.rx_handler = Some(handler_fn);
    }
}
