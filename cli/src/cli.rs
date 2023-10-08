use bluez_async::MacAddress;
use clap::{Parser, Subcommand};
use either::Either;
use std::path::PathBuf;

/// Object Transfer Service client
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Discovery for N seconds
    #[arg(short = 't', long)]
    pub disco: Option<u32>,

    /// Adapter address or name to use
    #[arg(short, long, value_parser = mac_or_name)]
    pub adapter: Option<Either<MacAddress, String>>,

    /// Device name or address to connect to
    #[arg(short, long, value_parser = mac_or_name)]
    pub device: Either<MacAddress, String>,

    /// Client action to do
    #[command(subcommand)]
    pub action: Action,
}

fn mac_or_name(val: &str) -> Result<Either<MacAddress, String>, String> {
    Ok(val
        .parse()
        .map(Either::Left)
        .ok()
        .unwrap_or_else(|| Either::Right(val.into())))
}

#[derive(Subcommand, Debug)]
pub enum Action {
    /// Get list of objects
    List(ListArgs),

    /// Read object data
    Read(ReadArgs),

    /// Write object data
    Write(WriteArgs),
}

/// Object list flags
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct ListArgs {
    /// Show object ids
    #[arg(short, long)]
    pub id: bool,

    /// Show object names
    #[arg(short, long)]
    pub name: bool,

    /// Show object types
    #[arg(short, long = "type")]
    pub type_: bool,

    /// Show current object sizes
    #[arg(short, long)]
    pub cur_size: bool,

    /// Show allocated object sizes
    #[arg(short, long)]
    pub alloc_size: bool,

    /// Show object sizes
    #[arg(short, long)]
    pub size: bool,

    /// Show object properties
    #[arg(short, long)]
    pub props: bool,

    /// Show full metadata
    #[arg(short, long)]
    pub full: bool,
}

macro_rules! list_flags {
    ( $( $name:ident: $($flag:ident)*; )* ) => {
        impl ListArgs {
            $(
                pub fn $name(&self) -> bool {
                    false $(|| self.$flag)*
                }
            )*
        }
    };
}

list_flags! {
    id: id full;
    name: name full;
    type_: type_ full;
    cur_size: cur_size size full;
    alloc_size: alloc_size size full;
    any_size: cur_size alloc_size size full;
    props: props full;
}

/// Object selection
#[derive(Parser, Debug)]
pub struct ObjSel {
    /// Object index
    #[arg(short = 'x', long)]
    pub index: Option<usize>,

    /// Object id
    #[arg(short, long)]
    pub id: Option<u64>,

    /// Object name
    #[arg(short, long)]
    pub name: Option<String>,
}

/// Data range selection
#[derive(Parser, Debug)]
pub struct RangeSel {
    /// Offset in bytes
    #[arg(short, long, default_value_t = 0)]
    pub offset: usize,

    /// Length in bytes [default: current size]
    #[arg(short, long)]
    pub length: Option<usize>,
}

/// Object read options
#[derive(Parser, Debug)]
pub struct ReadArgs {
    /// Object to read
    #[command(flatten)]
    pub object: ObjSel,

    /// Data slice to read
    #[command(flatten)]
    pub range: RangeSel,

    /// File to output data
    ///
    /// If file is set the binary data will be written to.
    /// Otherwise the hex data will be printed to stdout.
    #[arg(short, long)]
    pub file: Option<PathBuf>,
}

/// Object write options
#[derive(Parser, Debug)]
pub struct WriteArgs {
    /// Object to write
    #[command(flatten)]
    pub object: ObjSel,

    /// Data slice to write
    #[command(flatten)]
    pub range: RangeSel,

    /// Truncate data size after write
    #[arg(short, long)]
    pub truncate: bool,

    /// File to input data
    ///
    /// If file is set the binary data will be read from.
    /// Otherwise the hex data will be read from stdin.
    #[arg(short, long)]
    pub file: Option<PathBuf>,
}
