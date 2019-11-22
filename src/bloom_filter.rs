
pub struct BloomFilter {
    map: Vec<u8>,
    rolling: usize
}

impl BloomFilter {
    pub fn new(size: usize) -> BloomFilter {
        BloomFilter {
            map: vec![0; size],
            rolling: 0
        }
    }

    pub fn increment_cell(&mut self, mut cell: usize) -> bool {
//        println!("{}", cell);
        let res = self.map[cell] == 1;
        self.map[cell] = 1;
        res
//        cell %= (self.map.len() * 4) as u64;
//        let shift = ((cell % 4) * 2) as u8;
//        let map_cell = &mut self.map[(cell as usize) / 4];
//
//        let value = (*map_cell >> shift) & 0b11;
//        if value == 0b11 {
//            false
//        }
//        else {
//            *map_cell = (*map_cell & !(0b11 << shift)) | ((value + 1) << shift);
//            true
//        }
    }
}