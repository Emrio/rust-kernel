mod hardware_type;
mod operation;
mod protocol_type;

pub use hardware_type::HardwareType;
pub use operation::Operation as ARPOperation;
pub use protocol_type::ProtocolType;

use crate::net::l3_address::IPv4Address;

use super::error::BufferTooSmall;
use super::ethernet::address::EthernetAddress;

pub struct ARPPacket<T: AsRef<[u8]>> {
    buffer: T,
}

mod field {
    pub const HW_TYPE: core::ops::Range<usize> = 0..2;
    pub const PROTO_TYPE: core::ops::Range<usize> = 2..4;
    pub const HW_LENGTH: usize = 4;
    pub const PROTO_LENGTH: usize = 5;
    pub const OPERATION: core::ops::Range<usize> = 6..8;
    pub const SND_HW_ADDR: core::ops::Range<usize> = 8..14;
    pub const SND_PROTO_ADDR: core::ops::Range<usize> = 14..18;
    pub const TGT_HW_ADDR: core::ops::Range<usize> = 18..24;
    pub const TGT_PROTO_ADDR: core::ops::Range<usize> = 24..28;
}

pub const ARP_PACKET: usize = 28;

impl<T: AsRef<[u8]>> ARPPacket<T> {
    pub fn new_unchecked(buffer: T) -> Self {
        Self { buffer }
    }

    fn check_length(&self) -> Result<(), BufferTooSmall> {
        if self.buffer.as_ref().len() < ARP_PACKET {
            Err(BufferTooSmall)
        } else {
            Ok(())
        }
    }

    pub fn new(buffer: T) -> Result<Self, BufferTooSmall> {
        let frame = Self::new_unchecked(buffer);
        frame.check_length()?;
        Ok(frame)
    }

    pub fn into_inner(self) -> T {
        self.buffer
    }

    pub fn hardware_type(&self) -> HardwareType {
        HardwareType::from_bytes(&self.buffer.as_ref()[field::HW_TYPE])
    }

    pub fn protocol_type(&self) -> ProtocolType {
        ProtocolType::from_bytes(&self.buffer.as_ref()[field::PROTO_TYPE])
    }

    pub fn hardware_length(&self) -> u8 {
        self.buffer.as_ref()[field::HW_LENGTH]
    }

    pub fn protocol_length(&self) -> u8 {
        self.buffer.as_ref()[field::PROTO_LENGTH]
    }

    pub fn operation(&self) -> ARPOperation {
        ARPOperation::from_bytes(&self.buffer.as_ref()[field::OPERATION])
    }

    pub fn sender_hardware_address(&self) -> EthernetAddress {
        EthernetAddress::from_bytes(&self.buffer.as_ref()[field::SND_HW_ADDR])
    }

    pub fn sender_protocol_address(&self) -> IPv4Address {
        IPv4Address::from_bytes(&self.buffer.as_ref()[field::SND_PROTO_ADDR])
    }

    pub fn target_hardware_address(&self) -> EthernetAddress {
        EthernetAddress::from_bytes(&self.buffer.as_ref()[field::TGT_HW_ADDR])
    }

    pub fn target_protocol_address(&self) -> IPv4Address {
        IPv4Address::from_bytes(&self.buffer.as_ref()[field::TGT_PROTO_ADDR])
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> ARPPacket<T> {
    pub fn set_hardware_type(&mut self, hw_type: HardwareType) -> &mut Self {
        self.buffer.as_mut()[field::HW_TYPE].copy_from_slice(&hw_type.as_bytes());
        self
    }

    pub fn set_protocol_type(&mut self, protocol_type: ProtocolType) -> &mut Self {
        self.buffer.as_mut()[field::PROTO_TYPE].copy_from_slice(&protocol_type.as_bytes());
        self
    }

    pub fn set_hardware_length(&mut self, hardware_length: u8) -> &mut Self {
        self.buffer.as_mut()[field::HW_LENGTH] = hardware_length;
        self
    }

    pub fn set_protocol_length(&mut self, protocol_length: u8) -> &mut Self {
        self.buffer.as_mut()[field::PROTO_LENGTH] = protocol_length;
        self
    }

    pub fn set_operation(&mut self, operation: ARPOperation) -> &mut Self {
        self.buffer.as_mut()[field::OPERATION].copy_from_slice(&operation.as_bytes());
        self
    }

    pub fn set_sender_hardware_address(&mut self, address: EthernetAddress) -> &mut Self {
        self.buffer.as_mut()[field::SND_HW_ADDR].copy_from_slice(address.as_bytes());
        self
    }

    pub fn set_sender_protocol_address(&mut self, address: IPv4Address) -> &mut Self {
        self.buffer.as_mut()[field::SND_PROTO_ADDR].copy_from_slice(address.as_bytes());
        self
    }

    pub fn set_target_hardware_address(&mut self, address: EthernetAddress) -> &mut Self {
        self.buffer.as_mut()[field::TGT_HW_ADDR].copy_from_slice(address.as_bytes());
        self
    }

    pub fn set_target_protocol_address(&mut self, address: IPv4Address) -> &mut Self {
        self.buffer.as_mut()[field::TGT_PROTO_ADDR].copy_from_slice(address.as_bytes());
        self
    }
}

impl<T: AsRef<[u8]>> core::fmt::Display for ARPPacket<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "ARP(htype={}, hlen={}, prototype={}, protolen={}, operation={}, sender=({}, {}), target=({}, {}))",
            self.hardware_type(),
            self.hardware_length(),
            self.protocol_type(),
            self.protocol_length(),
            self.operation(),
            self.sender_hardware_address(),
            self.sender_protocol_address(),
            self.target_hardware_address(),
            self.target_protocol_address()
        ))
    }
}

#[cfg(test)]
mod test {
    use crate::net::ethernet::address::EthernetAddress;
    use crate::net::l3_address::IPv4Address;

    use super::{ARP_PACKET, ARPOperation, ARPPacket, HardwareType, ProtocolType};

    #[test_case]
    fn test_arp_request() {
        let mut packet = [0; ARP_PACKET];

        let mut arp = ARPPacket::new(&mut packet).unwrap();
        arp.set_hardware_type(HardwareType::Ethernet)
            .set_protocol_type(ProtocolType::IPv4)
            .set_hardware_length(EthernetAddress::SIZE as u8)
            .set_protocol_length(IPv4Address::SIZE as u8)
            .set_operation(ARPOperation::Request)
            .set_sender_hardware_address(EthernetAddress::from_bytes(&[
                0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
            ]))
            .set_sender_protocol_address(IPv4Address::new(10, 0, 2, 3))
            .set_target_hardware_address(EthernetAddress::BROADCAST)
            .set_target_protocol_address(IPv4Address::new(10, 0, 2, 2));

        assert_eq!(
            packet,
            [
                0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
                0x0a, 0x00, 0x02, 0x03, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0a, 0x00, 0x02, 0x02,
            ]
        )
    }

    #[test_case]
    fn test_arp_reply() {
        let packet = [
            0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02, 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02,
            0x0a, 0x00, 0x02, 0x02, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x0a, 0x00, 0x02, 0x03,
        ];
        let arp = ARPPacket::new(packet).unwrap();
        assert_eq!(arp.hardware_type(), HardwareType::Ethernet);
        assert_eq!(arp.protocol_type(), ProtocolType::IPv4);
        assert_eq!(arp.hardware_length(), 6);
        assert_eq!(arp.protocol_length(), 4);
        assert_eq!(arp.operation(), ARPOperation::Reply);
        assert_eq!(
            arp.sender_hardware_address(),
            EthernetAddress::from_bytes(&[0x52, 0x55, 0x0a, 0x00, 0x02, 0x02])
        );
        assert_eq!(
            arp.sender_protocol_address(),
            IPv4Address::from_bytes(&[10, 0, 2, 2])
        );
        assert_eq!(
            arp.target_hardware_address(),
            EthernetAddress::from_bytes(&[0x52, 0x54, 0x00, 0x12, 0x34, 0x56])
        );
        assert_eq!(
            arp.target_protocol_address(),
            IPv4Address::from_bytes(&[10, 0, 2, 3])
        );
    }
}
