use crate::sequences_reader::{DnaSequence, SequencesReader};
use crate::sequences_stream::{GenericSequencesStream, SequenceInfo};
use std::path::PathBuf;

pub struct FastaFileSequencesStream {
    sequences_reader: SequencesReader,
}

impl FastaFileSequencesStream {
    pub fn get_estimated_bases_count(file: &PathBuf) -> u64 {
        // TODO: Improve this ratio estimation
        const COMPRESSED_READS_RATIO: f64 = 0.5;

        let length = std::fs::metadata(file)
            .expect(&format!("Error while opening file {}", file.display()))
            .len();

        let file_bases_count = if file
            .extension()
            .map(|x| x == "gz" || x == "lz4")
            .unwrap_or(false)
        {
            (length as f64 * COMPRESSED_READS_RATIO) as u64
        } else {
            length
        };
        file_bases_count
    }
}

impl GenericSequencesStream for FastaFileSequencesStream {
    type SequenceBlockData = PathBuf;

    fn new() -> Self {
        Self {
            sequences_reader: SequencesReader::new(),
        }
    }

    fn read_block(
        &mut self,
        block: &Self::SequenceBlockData,
        copy_ident_data: bool,
        partial_read_copyback: Option<usize>,
        mut callback: impl FnMut(DnaSequence, SequenceInfo),
    ) {
        self.sequences_reader.process_file_extended(
            block,
            |x| callback(x, SequenceInfo { color: None }),
            partial_read_copyback,
            copy_ident_data,
            false,
        );
    }
}
