# NIB Archive Decoder/Encoder

Decode and encode NIB Archive `.nib` files.

The `.nib` file is mainly used by Interface Builder component of Xcode to encode `.xib` files.
Both store information about creating a GUI for macOS and iOS applications.
The difference is, `.xib` is a human-readable xml used only during development,
and a `.nib` is a compiled version of it.

There're two versions of `.nib`s. The first one is a NIB Archive (used by UIKit
on iPhones since iOS 6) whose decoded structure somewhat resembles Cocoa Keyed
Archive. The second one is actually a Cocoa Keyed Archive (used prior iOS 6).
macOS uses both versions.

The file format has been described in detail in the
[nibsqueeze](https://github.com/matsmattsson/nibsqueeze/blob/master/NibArchive.md) repository
and this great [article](https://www.mothersruin.com/software/Archaeology/reverse/uinib.html).
You may also want to check the [nibarchive](https://github.com/MatrixEditor/nibarchive) repository –
a NIB Archive parser written in Python.

## Known issues

Some NIB Archives (presumably ones with a coder version of 10) have some extra bytes
at the end of a file. Those bytes are not handled and their purpose is unknown yet.

## Example

The following example prints all archive's objects and their values:

```rust
use nibarchive::*;

let archive: NIBArchive = NIBArchive::from_file("./foo.nib")?;

for (i, object) in 0..archive.objects().iter().enumerate() {
    let class_name = object.class_name(&archive.class_names()).name();
    println!("[{i}] Object of a class '{class_name}':");

    let values: &[Value] = object.values(&archive.values());
    for (j, value) in 0..values.iter().enumerate() {
        let key = value.key(&archive.keys());
        let inner_value = value.value();
        println!("-- [{j}] {key}: {inner_value:?}");
    }
}
```
