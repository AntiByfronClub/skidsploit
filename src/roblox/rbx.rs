use crate::skid;
use crate::offsets;

const CHILD_LIMIT: usize = 0x4000;
const DEFAULT_STRING_LEN: usize = 256;
const CLASS_STRING_LEN: usize = 128;

pub type revertFn = Box<dyn FnOnce() + Send>;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct instance {
    pub address: usize,
}

impl instance {
    pub fn new(address: usize) -> Self {
        Self { address }
    }

    pub fn nonZero(&self) -> bool {
        self.address != 0
    }

    pub fn name(&self) -> String {
        if !self.nonZero() {
            return String::new();
        }
        let k = skid::skid();
        let container = k.read::<usize>(self.address + offsets::instance::name);
        if container == 0 {
            return String::new();
        }
        let namePtr = container + offsets::instance::nameStr;
        let data = k.read::<usize>(namePtr);
        if data != 0 {
            let value = k.readString(data, DEFAULT_STRING_LEN);
            if !value.is_empty() {
                return value;
            }
        }
        k.readString(namePtr, DEFAULT_STRING_LEN)
    }

    pub fn className(&self) -> String {
        if !self.nonZero() {
            return String::new();
        }
        let k = skid::skid();
        let desc = k.read::<usize>(self.address + offsets::instance::classDesc);
        if desc == 0 {
            return String::new();
        }
        let namePtr = k.read::<usize>(desc + offsets::instance::className);
        if namePtr == 0 {
            return String::new();
        }
        k.readString(namePtr, CLASS_STRING_LEN)
    }

    pub fn children(&self) -> Vec<instance> {
        let mut out = Vec::new();
        if !self.nonZero() {
            return out;
        }
        let k = skid::skid();
        let container = k.read::<usize>(self.address + offsets::instance::childrenStart);
        if container == 0 {
            return out;
        }
        let begin = k.read::<usize>(container);
        let end = k.read::<usize>(container + offsets::instance::childrenEnd);
        if begin == 0 || end == 0 || end <= begin {
            return out;
        }
        let count = (end - begin) / std::mem::size_of::<usize>();
        if count > CHILD_LIMIT {
            return out;
        }
        out.reserve(count);
        for index in 0..count {
            let child = k.read::<usize>(begin + index * std::mem::size_of::<usize>());
            if child != 0 {
                out.push(instance::new(child));
            }
        }
        out
    }

    pub fn findFirstChild(&self, target: &str) -> instance {
        for child in self.children() {
            if child.name() == target {
                return child;
            }
        }
        instance::default()
    }

    pub fn find(&self, path: &[&str]) -> instance {
        let mut current = *self;
        for part in path {
            if !current.nonZero() {
                return instance::default();
            }
            current = current.findFirstChild(part);
        }
        current
    }

    pub fn spoofTo(&self, target: instance) -> bool {
        skid::skid().write(self.address + 0x8, &target.address)
    }

    pub fn getScriptContext(&self) -> instance {
        if !self.nonZero() {
            return instance::default();
        }
        let k = skid::skid();
        let children = k.read::<usize>(self.address + offsets::instance::childrenStart);
        if children == 0 {
            return instance::default();
        }
        let firstChild = k.read::<usize>(children);
        if firstChild == 0 {
            return instance::default();
        }
        instance::new(k.read::<usize>(firstChild + offsets::datamodel::scriptContext))
    }

    pub fn setBypass(&self, value: bool) -> bool {
        skid::skid().write(self.address + offsets::datamodel::requireBypass, &value)
    }

    pub fn setBytecode(&self, bytecode: &[u8]) -> Option<revertFn> {
        if !self.nonZero() || bytecode.is_empty() {
            return None;
        }
        let k = skid::skid();
        let parent = k.read::<usize>(self.address + offsets::script::moduleScript::pointer);
        if parent == 0 {
            return None;
        }

        let oldBuffer = k.read::<usize>(parent + offsets::script::pointer);
        let oldSize = k.read::<usize>(parent + offsets::script::size);

        let newBuffer = k.allocate(bytecode.len());
        if newBuffer == 0 {
            return None;
        }

        if !k.writeRaw(newBuffer, bytecode.as_ptr(), bytecode.len()) {
            k.free(newBuffer);
            return None;
        }

        let verified = k.readBytes(newBuffer, bytecode.len());
        if verified.len() != bytecode.len() || verified.as_slice() != bytecode {
            k.free(newBuffer);
            return None;
        }

        k.write(parent + offsets::script::pointer, &newBuffer);
        k.write(parent + offsets::script::size, &bytecode.len());

        let revertData = (parent, oldBuffer, oldSize, newBuffer);
        Some(Box::new(move || {
            let (parent, oldBuffer, oldSize, newBuffer) = revertData;
            let k = skid::skid();
            k.write(parent + offsets::script::pointer, &oldBuffer);
            k.write(parent + offsets::script::size, &oldSize);
            k.free(newBuffer);
        }))
    }

    pub fn unlockModule(&self) -> bool {
        let k = skid::skid();
        k.write(
            self.address + offsets::script::moduleScript::detection,
            &0x100000000usize,
        );
        k.write(self.address + offsets::script::moduleScript::coreScript, &0x2usize);
        true
    }
}
