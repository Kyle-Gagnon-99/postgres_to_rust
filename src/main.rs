use std::{
    collections::HashMap,
    env,
    fs::{self, File, OpenOptions},
    io::{Write, Read},
    path::Path,
    process::Command,
};

use clap::{command, Arg, ArgAction};
use convert_case::{Case, Casing};
use quote::{__private::Span, quote};
use syn::Ident;
use tracing::{debug, error, info, warn};
use tracing_subscriber::FmtSubscriber;

fn main() {
    let matches = command!()
        .arg(Arg::new("verbose")
            .long("verbose")
            .short('v')
            .help("Sets the level of verbosity")
            .required(false)
            .action(ArgAction::SetTrue)
        )
        .arg(Arg::new("env_file")
            .long("env-file")
            .help("Sets the environment file. This file is used if the environment variables are not set. Used over the username, password, host, and port arguments.")
            .required(false)
        )
        .arg(Arg::new("host")
            .long("host")
            .help("Sets the PostgreSQL host")
            .required(false)
            .default_value("localhost")
        )
        .arg(Arg::new("port")
            .long("port")
            .help("Sets the PostgreSQL port")
            .required(false)
            .default_value("5432")
        )
        .arg(Arg::new("username")
            .long("username")
            .help("Sets the PostgreSQL username")
            .required(false)
        )
        .arg(Arg::new("password")
            .long("password")
            .help("Sets the PostgreSQL password")
            .required(false)
        )
        .arg(Arg::new("database")
            .long("database")
            .help("Sets the PostgreSQL database")
            .required(true)
        )
        .arg(Arg::new("include_views")
            .short('i')
            .long("include-views")
            .help("Include PostgreSQL views in the generated schema")
            .required(false)
            .action(ArgAction::SetTrue)
        )
        .arg(Arg::new("schema")
            .short('s')
            .long("schema")
            .help("Sets the PostgreSQL schema")
            .required(false)
            .default_value("public")
        )
        .arg(Arg::new("table_file")
            .long("table-file")
            .help("Map a PostgreSQL table to a specific file. Format: 'table:file'. To map multiple table separate with a comma. Example: 'users:users,posts:posts'")
            .required(false)
            .action(ArgAction::Append)
        )
        .arg(Arg::new("uuid")
            .long("uuid")
            .help("Use UUIDs for columns of type uuid")
            .required(false)
            .action(ArgAction::SetTrue)
        )
        .arg(Arg::new("output_directory")
            .short('d')
            .long("output-directory")
            .help("Sets the output directory")
            .required(false)
            .default_value("src")
        )
        .arg(Arg::new("output")
            .short('o')
            .long("output")
            .help("Sets the output file")
            .required(false)
            .default_value("schema.rs")
        )
        .get_matches();

    // If the verbose flag is set, set the environment filter to debug, otherwise set it to info
    let verbose = matches.get_flag("verbose");
    let env_filter = match verbose {
        true => "debug",
        false => "info",
    };

    // Set up the tracing subscriber
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_env_filter(env_filter)
        .finish();

    // Set the global default subscriber
    tracing::subscriber::set_global_default(subscriber).unwrap();

    // Get the environment variables from the command line arguments or the environment file
    let env_file: Option<&String> = matches.get_one::<String>("env_file");

    // Get the PostgreSQL username
    let username = if let Some(env_file) = env_file {
        dotenv::from_filename(env_file).ok();
        dotenv::var("POSTGRES_USER").unwrap()
    } else if env::var("POSTGRES_USER").is_ok() {
        env::var("POSTGRES_USER").unwrap()
    } else {
        matches
            .get_one::<String>("username")
            .expect("POSTGRES_USER or username must be set")
            .to_string()
    };

    // Get the PostgreSQL password
    let password = if let Some(env_file) = env_file {
        dotenv::from_filename(env_file).ok();
        dotenv::var("POSTGRES_PASSWORD").unwrap()
    } else if env::var("POSTGRES_PASSWORD").is_ok() {
        env::var("POSTGRES_PASSWORD").unwrap()
    } else {
        matches
            .get_one::<String>("password")
            .expect("POSTGRES_PASSWORD or password must be set")
            .to_string()
    };

    // Get the PostgreSQL host
    let host = if let Some(env_file) = env_file {
        dotenv::from_filename(env_file).ok();
        dotenv::var("POSTGRES_HOST").unwrap()
    } else if env::var("POSTGRES_HOST").is_ok() {
        env::var("POSTGRES_HOST").unwrap()
    } else {
        matches
            .get_one::<String>("host")
            .expect("POSTGRES_HOST or host must be set")
            .to_string()
    };

    // Get the PostgreSQL port
    let port = if let Some(env_file) = env_file {
        dotenv::from_filename(env_file).ok();
        dotenv::var("POSTGRES_PORT").unwrap()
    } else if env::var("POSTGRES_PORT").is_ok() {
        env::var("POSTGRES_PORT").unwrap()
    } else {
        matches
            .get_one::<String>("port")
            .expect("POSTGRES_PORT or port must be set")
            .to_string()
    };

    // Get the PostgreSQL database
    let database = matches
        .get_one::<String>("database")
        .expect("Database must be set");

    // Get the PostgreSQL schema
    let schema = matches
        .get_one::<String>("schema")
        .expect("Schema must be set");

    // Get the output file
    let output_file = matches
        .get_one::<String>("output")
        .expect("Output must be set")
        .to_string();

    // Get the output directory
    let output_directory = matches
        .get_one::<String>("output_directory")
        .expect("Output directory must be set")
        .to_string();

    // Get the UUID flag
    let use_uuid = matches.get_flag("uuid");

    // Get the include views flag
    let _include_views = matches.get_flag("include_views");

    // Get the table file mappings
    let table_file_mappings = matches.get_one::<String>("table_file");

    // Create a HashMap of the table file mappings
    let table_file_mappings: HashMap<String, String> = match table_file_mappings {
        Some(table_file_map) => {
            let mut table_file_mappings = HashMap::new();
            for table_file in table_file_map.split(",") {
                let table_file: Vec<&str> = table_file.split(":").collect();
                if table_file.len() != 2 {
                    panic!("Please provide a table file mapping in the format 'table:file'");
                }
                table_file_mappings.insert(table_file[0].to_string(), table_file[1].to_string());
            }

            table_file_mappings
        }
        None => HashMap::new(),
    };

    let mut module_defs: Vec<String> = Vec::new();
    let mut output_file_contents: Vec<String> = Vec::new();
    let mut file_list: Vec<String> = Vec::new();

    // Print the table file mappings, if any
    if table_file_mappings.len() > 0 {
        debug!("Table file mappings:");
        for (table, file) in &table_file_mappings {
            debug!("{} -> {}/{}.rs", table, output_directory, file);
            // If the file does exist, delete it
            // We do this to ensure that the file is up to date
            let output_file_name = output_file.clone().replace(".rs", "");
            if Path::new(&format!("{}/{}/{}.rs", output_directory, output_file_name, file)).exists() {
                debug!("Deleting {}/{}/{}.rs", output_directory, output_file_name, file);
                fs::remove_file(format!("{}/{}/{}.rs", output_directory, output_file_name, file)).unwrap();
            }
        }
    }

    // Create the connection string
    let connection_string = format!(
        "postgres://{}:{}@{}:{}/{}",
        username, password, host, port, database
    );

    debug!("Connection string: {}", connection_string);
    info!("Connecting to PostgreSQL database");

    // Connect to the PostgreSQL database
    let mut client = match postgres::Client::connect(&connection_string, postgres::NoTls) {
        Ok(client) => client,
        Err(error) => {
            panic!("Failed to connect to PostgreSQL database: {}", error);
        }
    };

    info!("Connected to PostgreSQL database");
    // Get the tables from the database
    let tables = client.query("SELECT table_name FROM information_schema.tables WHERE table_schema = $1 AND table_type = 'BASE TABLE'", &[&schema]);
    let tables = match tables {
        Ok(tables) => tables,
        Err(error) => {
            panic!("Failed to query tables: {}", error);
        }
    };

    // Set up the tables vector
    for rows in tables {
        let table_name: String = rows.get(0);
        info!("Generating schema for table {}", table_name);

        // Set up the fields for the Rust struct
        let mut fields = Vec::new();

        // Get the columns from the table
        let columns = client.query("SELECT column_name, data_type, is_nullable, column_default FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2", &[&schema, &table_name]);
        let columns = match columns {
            Ok(columns) => columns,
            Err(error) => {
                panic!("Failed to query columns: {}", error);
            }
        };

        // For each column, generate the Rust struct field
        for column in columns {
            let column_name: String = column.get(0);
            let data_type: String = column.get(1);
            let is_nullable: String = column.get(2);

            debug!("Generating schema for column {}", column_name);
            let rust_type = match data_type.as_str() {
                "bigint" => quote! { i64 },
                "bigserial" => quote! { i64 },
                "bit" => quote! { i8 },
                "bit varying" => quote! { i8 },
                "boolean" => quote! { bool },
                "box" => quote! { String },
                "bytea" => quote! { Vec<u8> },
                "character" => quote! { String },
                "character varying" => quote! { String },
                "cidr" => quote! { String },
                "circle" => quote! { String },
                "date" => quote! { chrono::NaiveDate },
                "double precision" => quote! { f64 },
                "inet" => quote! { String },
                "integer" => quote! { i32 },
                "interval" => quote! { String },
                "json" => quote! { serde_json::Value },
                "jsonb" => quote! { serde_json::Value },
                "line" => quote! { String },
                "lseg" => quote! { String },
                "macaddr" => quote! { String },
                "money" => quote! { String },
                "numeric" => quote! { f64 },
                "path" => quote! { String },
                "pg_lsn" => quote! { String },
                "point" => quote! { String },
                "polygon" => quote! { String },
                "real" => quote! { f32 },
                "smallint" => quote! { i16 },
                "smallserial" => quote! { i16 },
                "serial" => quote! { i32 },
                "text" => quote! { String },
                "timestampz" => quote! { String },
                "uuid" => match use_uuid {
                    true => quote! { uuid::Uuid },
                    false => quote! { String },
                },
                _ => quote! { String },
            };

            // If the column has a default value, set the Rust type to an Option
            let rust_type = if is_nullable == "YES" {
                quote! { Option<#rust_type> }
            } else {
                rust_type
            };

            // Convert the column name to snake case
            let column_name = column_name.to_case(Case::Snake);
            let column_name = Ident::new(&column_name, Span::call_site());
            let column_name = quote!(#column_name);

            // Add the field to the fields vector
            fields.push(quote! {
                pub #column_name: #rust_type,
            });
        }

        // Generate the Rust struct
        let struct_name = table_name.to_case(Case::Pascal);
        let struct_name = Ident::new(&struct_name, Span::call_site());
        let struct_name = quote!(#struct_name);

        // Generate the struct definition
        let struct_definition = quote! {
            #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
            pub struct #struct_name {
                #(#fields)*
            }
        };

        // If the user wants to generate a file for each table, do so
        if let Some(file_path) =
            table_file_mappings.get(&struct_name.to_string().to_case(Case::Snake))
        {
            // Get the full name of the file
            // Get the name of the output file but replace the .rs extension with an empty string
            let output_file_name = output_file.clone().replace(".rs", "");
            let file_path = format!("{}/{}/{}.rs", output_directory, output_file_name, file_path);
            debug!("Writing struct definition to {}", file_path);

            // Create the file if it doesn't exist
            // Create the directory if it doesn't exist
            if !Path::new(&file_path).exists() {
                let dir_path = Path::new(&file_path).parent().unwrap();
                if !dir_path.exists() {
                    fs::create_dir_all(dir_path).unwrap();
                }
                File::create(&file_path).unwrap();
            }

            // Create the file, in append mode
            let mut file = OpenOptions::new()
                .write(true)
                .append(true)
                .open(&file_path)
                .unwrap();

            // Write the struct definition to the file
            write!(file, "{}\n", struct_definition).unwrap();

            // Add the file to the list of files to be formatted
            file_list.push(file_path.clone());

            // Add the file to the list of modules, replacing the .rs extension with an empty string
            // and replacing the / with a :: to create a module path, but don't add the root module
            let module_name = file_path
                .replace(".rs", "")
                .replace("/", "::")
                .replace(&format!("{}::", output_directory), "")
                .replace(&format!("{}::", output_file_name), "");

            module_defs.push(format!("pub mod {};", module_name));
        } else {
            output_file_contents.push(struct_definition.to_string());
        }
    }

    // Create the output file
    let output = format!("{}/{}", output_directory, output_file);
    let mut file = File::create(&output).unwrap();

    // Write a header to the file
    write!(file, "// This file was generated by rustgres-schema\n").unwrap();
    write!(file, "// Do not edit this file directly\n").unwrap();
    // Add a timestamp to the file, in the format of YYYY-MM-DD HH:MM:SS
    let timestamp = chrono::offset::Local::now().format("%Y-%m-%d %H:%M:%S");
    write!(file, "// Generated on {}\n", timestamp).unwrap();

    // Write the module definitions to the file
    for module_def in module_defs {
        write!(file, "{}\n", module_def).unwrap();
    }

    for line in output_file_contents {
        write!(file, "{}\n", line).unwrap();
    }

    // Run rustfmt on the list of files. Check to see if the files exist first
    for file in file_list {
        if Path::new(&file).exists() {

            // First remove whitespaces around ::
            let mut file_contents = String::new();
            let mut file_to_open = File::open(&file).unwrap();
            file_to_open.read_to_string(&mut file_contents).unwrap();
            let file_contents = file_contents.replace(" :: ", "::");
            let mut file_to_open = File::create(&file).unwrap();
            file_to_open.write_all(file_contents.as_bytes()).unwrap();

            debug!("Running rustfmt on {}", file);
            // Run rustfmt on the output file
            match Command::new("rustfmt").arg(&file).output() {
                Ok(_) => {
                    debug!("Ran rustfmt on {}", file);
                }
                Err(_) => {
                    warn!("Rustfmt not found, skipping formatting")
                }
            }
        }
    }

    match client.close() {
        Ok(_) => {
            info!("Closed PostgreSQL connection");
        }
        Err(error) => {
            error!("Failed to close PostgreSQL connection: {}", error);
            panic!("Failed to close PostgreSQL connection: {}", error);
        }
    }
}
