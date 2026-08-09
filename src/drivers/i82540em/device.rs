use x86_64::{PhysAddr, instructions::hlt};

use crate::{
    drivers::i82540em::{
        I8254_CTRL_ASDE, I8254_CTRL_RESET, I8254_CTRL_SLU, I8254_EERD_DONE, I8254_REG_CTRL,
        I8254_REG_EERD, I8254_REG_RAH, I8254_REG_RAL,
        tx::{
            CMD_EOP, CMD_IFCS, CMD_RS, REG_TDH, REG_TDT, STA_DD, TX_BUFFERS, TX_DESCS, TX_SIZE,
            TxDescriptor,
        },
    },
    memory::{MEMORY_MAPPER, MemoryMapper},
    mmio::MmioPtr,
    net::{device::NetworkDevice, ethernet::address::EthernetAddress},
};

pub struct Device {
    base_address: MmioPtr<u32>,
    hardware_address: EthernetAddress,
}

impl Device {
    pub(super) fn from(bar0: u32) -> Self {
        let mapper = MEMORY_MAPPER
            .get()
            .expect("memory mapper to be initialized");

        let base_address = mapper.get_virt_mut(PhysAddr::new((bar0 & 0xfffffff8u32) as u64));

        Self {
            base_address: MmioPtr::new(base_address),
            hardware_address: EthernetAddress::BROADCAST,
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
}

impl NetworkDevice for Device {
    fn send_packet(&self, buffer: &[u8]) {
        let mapper = MEMORY_MAPPER
            .get()
            .expect("memory mapper to be initialized");

        let tail = self.read_register(REG_TDT) as usize;
        let head = self.read_register(REG_TDH) as usize;

        if (tail + 1) % TX_SIZE == head {
            kprintln!("Cannot send packet: buffer full");
        }

        unsafe {
            TX_BUFFERS[tail][0..buffer.len()].copy_from_slice(buffer);

            let descriptor = &raw mut TX_DESCS[tail];
            descriptor.write_volatile(TxDescriptor {
                buffer_address: mapper.get_physical(&raw const TX_BUFFERS[tail]),
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
}
