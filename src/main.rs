use anyhow::anyhow;
use argh::FromArgs;
use nibarchive::json::nib_to_json;
use nibarchive::NIBArchive;
use std::path::PathBuf;
use std::process::exit;

#[derive(FromArgs, PartialEq, Debug)]
/// Decode and encode NIB Archive `.nib` files.
///
/// # Examples
/// nibarchive tojson "path/to/file.nib" "path/to/output.json"
struct Opts {
    #[argh(subcommand)]
    commands: Commands,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Commands {
    ToJson(ToJsonOpts),
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "tojson")]
/// Decode a `.nib` file to JSON.
struct ToJsonOpts {
    #[argh(positional)]
    /// the path to the `.nib` file to decode
    input: PathBuf,

    #[argh(positional)]
    /// the path to the output json file
    output: PathBuf,
}

fn main_inner() -> Result<(), anyhow::Error> {
    let opts = argh::from_env::<Opts>();

    match opts.commands {
        Commands::ToJson(extract_opts) => {
            let archive = NIBArchive::from_file(&extract_opts.input).map_err(|err| {
                anyhow!(
                    "Failed to open NIB archive {:?}: {}",
                    extract_opts.input,
                    err
                )
            })?;

            // convert the archive to json
            let json = nib_to_json(archive)?;

            // convert the json to a string
            let json_string = serde_json::to_string_pretty(&json)?;

            // create the parent directories if they don't exist
            if let Some(parent) = extract_opts.output.parent() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    anyhow!(
                        "Failed to create parent directories for {:?}: {}",
                        extract_opts.output,
                        err
                    )
                })?;
            }

            // write the json to the output file
            std::fs::write(&extract_opts.output, json_string).map_err(|err| {
                anyhow!("Failed to write JSON to {:?}: {}", extract_opts.output, err)
            })?;
        }
    }

    Ok(())
}

fn main() {
    if let Err(err) = main_inner() {
        eprintln!("Error: {}", err);
        exit(1);
    }
}
