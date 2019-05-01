use std::error::Error;
use std::fmt;
use std::fs::File;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::slf::{SlfEntryState, SlfHeader};

// A pack of game resources.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ResourcePack {
    // A friendly name of the resource pack for display purposes.
    pub name: String,

    // The properties of the resource.
    //
    // # Properties
    //
    //  * with_archive_slf (bool, default = false) - the contents of SLF archives are included
    pub properties: Map<String, Value>,

    // The resources in this pack.
    pub resources: Vec<Resource>,
}

// A resource in the pack.
//
// A resource always maps to raw data, which can be a file or data inside an archive.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Resource {
    // The identity of the resource as a relative path.
    pub path: String,

    // The properties of the resource.
    //
    // # Properties
    //
    //  * archive_path (string) - the resource path of the archive that contains this resource
    //  * archive_slf (bool) - true if the resource is a SLF archive
    //  * archive_slf_num_resources (number) - number of resources inside the SLF archive
    pub properties: Map<String, Value>,
}

#[derive(Debug)]
struct ResourceError {
    desc: String,
}

impl ResourcePack {
    // Adds resources to the pack.
    //
    // The contents of the directory will be added (recursive).
    // The resource paths will be relative to the directory.
    // This should be called with the path to the "Data" directory of the game.
    #[allow(dead_code)]
    pub fn add_dir(&mut self, dir: &Path) -> Result<(), Box<Error>> {
        if !dir.is_dir() {
            return Err(Box::new(ResourceError::new(format!(
                "{:?} is not an accessible directory",
                dir
            ))));
        }
        self.add_sub_path(dir, dir)?;
        return Ok(());
    }

    // Adds resources to the pack.
    //
    // If the path points to a directory, the contents will be added (recursive).
    // If the path points to a file, the file will be added.
    // The resource paths will be relative to base.
    #[allow(dead_code)]
    pub fn add_sub_path(&mut self, base: &Path, path: &Path) -> Result<(), Box<Error>> {
        // must have a valid resource path
        let my_path = resource_path(base, path)?;
        if path.is_dir() {
            for entry in path.read_dir()? {
                if let Ok(entry) = entry {
                    self.add_sub_path(base, &entry.path())?;
                }
            }
        } else if path.is_file() {
            let mut resource = Resource::default();
            resource.path = my_path;
            // include slf contents
            if self.get_bool("with_archive_slf").unwrap_or(false) {
                if uppercase_extension(&path) == "SLF" {
                    resource.set_property("archive_slf", true);
                    let num_resources = self.add_slf(&path, &resource.path)?;
                    resource.set_property("archive_slf_num_resources", num_resources);
                }
            }
            self.resources.push(resource);
        } else {
            return Err(Box::new(ResourceError::new(format!(
                "{:?} in not an accessible directory or file",
                path
            ))));
        }
        return Ok(());
    }

    // Adds resources to the pack.
    //
    // The Ok entries of the SLF archive will be added as resources.
    // The resources will have the archive_path property set.
    // Returns the number of resources added.
    #[allow(dead_code)]
    fn add_slf(&mut self, path: &Path, archive_path: &str) -> Result<i32, Box<Error>> {
        let mut f = File::open(path)?;
        let header = SlfHeader::from_input(&mut f)?;
        let mut num_resources = 0;
        for entry in header.entries_from_input(&mut f)? {
            match entry.state {
                SlfEntryState::Ok => {
                    let mut resource = Resource::default();
                    let path = header.library_path.clone() + &entry.file_path;
                    resource.path = path.replace("\\", "/");
                    resource.set_property("archive_path", archive_path);
                    // TODO include archive inside archive?
                    self.resources.push(resource);
                    num_resources += 1;
                }
                _ => {}
            }
        }
        return Ok(num_resources);
    }
}

impl ResourceError {
    fn new(desc: String) -> Self {
        return ResourceError { desc };
    }
}

impl Error for ResourceError {
    fn description(&self) -> &str {
        return self.desc.as_str();
    }
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ResourceError({:?})", self.desc)
    }
}

// Gets the extension of the path in uppercase.
// Assumes the extension only contains valid utf8.
fn uppercase_extension(path: &Path) -> String {
    if let Some(extension) = path.extension() {
        return extension.to_str().unwrap().to_uppercase();
    }
    return "".to_string();
}

// Converts an OS path to a resource path.
fn resource_path(base: &Path, path: &Path) -> Result<String, Box<Error>> {
    let sub_path = path.strip_prefix(base)?;
    return match sub_path.to_str() {
        Some(s) => Ok(s.replace("\\", "/")),
        None => Err(Box::new(ResourceError::new(format!(
            "{:?} contains invalid utf8",
            sub_path
        )))),
    };
}

// Trait the adds shortcuts for properties.
pub trait Properties {
    // Gets a reference to the properties container.
    fn properties(&self) -> &Map<String, Value>;

    // Gets a mutable reference to the properties container.
    fn properties_mut(&mut self) -> &mut Map<String, Value>;

    // Removes a property and returns the old value.
    fn remove_property(&mut self, name: &str) -> Option<Value> {
        return self.properties_mut().remove(name);
    }

    // Sets the value of a property and returns the old value.
    fn set_property<T: Serialize>(&mut self, name: &str, value: T) -> Option<Value> {
        return self.properties_mut().insert(name.to_owned(), json!(value));
    }

    // Gets a property value.
    fn get_property(&self, name: &str) -> Option<&Value> {
        return self.properties().get(name);
    }

    // Gets a bool property value.
    fn get_bool(&self, name: &str) -> Option<bool> {
        return self.get_property(name).and_then(|v| v.as_bool());
    }

    // Gets a string property value.
    fn get_str(&self, name: &str) -> Option<&str> {
        return self.get_property(name).and_then(|v| v.as_str());
    }

    // Gets a signed integer property value.
    fn get_i64(&self, name: &str) -> Option<i64> {
        return self.get_property(name).and_then(|v| v.as_i64());
    }

    // Gets an unsigned integer property value.
    fn get_u64(&self, name: &str) -> Option<u64> {
        return self.get_property(name).and_then(|v| v.as_u64());
    }

    // Gets a floating-point property value.
    fn get_f64(&self, name: &str) -> Option<f64> {
        return self.get_property(name).and_then(|v| v.as_f64());
    }
}

impl Properties for ResourcePack {
    fn properties(&self) -> &Map<String, Value> {
        return &self.properties;
    }

    fn properties_mut(&mut self) -> &mut Map<String, Value> {
        return &mut self.properties;
    }
}

impl Properties for Resource {
    fn properties(&self) -> &Map<String, Value> {
        return &self.properties;
    }

    fn properties_mut(&mut self) -> &mut Map<String, Value> {
        return &mut self.properties;
    }
}
