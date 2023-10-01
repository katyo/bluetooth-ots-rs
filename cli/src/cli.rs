use bluez_async::MacAddress;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Object Transfer Service client
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Discovery for N seconds
    #[arg(short, long)]
    pub disco: Option<u32>,

    /// Device address to connect to
    #[arg(short, long)]
    pub address: Option<MacAddress>,

    /// Device name to connect to
    #[arg(short, long)]
    pub name: Option<String>,

    /// Client action to do
    #[command(subcommand)]
    pub action: Action,
}

#[derive(Subcommand, Debug)]
pub enum Action {
    /// Get list of objects
    List(ListArgs),

    /// Read object data
    Read(ReadArgs),
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
