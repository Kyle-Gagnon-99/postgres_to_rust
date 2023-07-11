# rustgres-schema

rustgres-schema is a Rust binary that generates Rust code from a remote PostgreSQL database schema. This project is currently a work in progress but is usable.

## Usage

To see the available options, run:
```
cargo run -- --help
```

## Example

To generate a Rust file from a local PostgreSQL database, run:
```
cargo run -- --host localhost --port 5432 --user postgres --password postgres --database postgres --schema public --output src/schema.rs
```

To map a table to a Rust file, run:
```
cargo run -- --host localhost --port 5432 --user postgres --password postgres --database postgres --schema public --table-list profiles:profiles,users:users --output-directory src --output-file schema.rs
```

If a table named profiles exists in the public schema, then profiles will be mapped to `src/[output file]/profiles.rs`. So in this case `src/schema.rs/profiles.rs` will be created. The same goes for users.

In the output file, the modules will be created. So in this case, `src/schema.rs` will contain:
```rust
pub mod profiles;
pub mod users;
```

## TODO
A list of things that need to be done:
- [x] Generate Rust code from a PostgreSQL database schema
- [ ] Add support for more data types
- [ ] Add support for more PostgreSQL features
- [ ] Add tests
- [ ] Possible cargo book?

## License

rustgres-schema is licensed under the MIT license. See the LICENSE file for more information.