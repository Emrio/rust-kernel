pub trait Split {
    type Output;
    fn split(self) -> (Self::Output, Self::Output);
}

impl Split for u32 {
    type Output = u16;
    fn split(self) -> (u16, u16) {
        ((self >> 16) as u16, (self & 0xffff) as u16)
    }
}

impl Split for u16 {
    type Output = u8;
    fn split(self) -> (u8, u8) {
        ((self >> 8) as u8, (self & 0xff) as u8)
    }
}

pub trait SplitTwice {
    type Output;
    fn split_twice(self) -> (Self::Output, Self::Output, Self::Output, Self::Output);
}

impl SplitTwice for u32 {
    type Output = u8;
    fn split_twice(self) -> (Self::Output, Self::Output, Self::Output, Self::Output) {
        let (first, second) = self.split();
        let (a, b) = first.split();
        let (c, d) = second.split();
        (a, b, c, d)
    }
}
