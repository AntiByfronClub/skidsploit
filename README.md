<img width="1599" height="825" alt="Image" src="https://github.com/user-attachments/assets/2726ca28-d48a-4ba8-85b6-8e1c63d31a2c" />

---

hi skid.

this is a base so you can make your own shitsploits. and this base will try to upkeep with dyn signing updates and offsets though not perfect.

its very easy to update most offsets are public knowledge through websites.

star repo pls.

credits:
- @dyn for the signer
- @Okafor-twd (HOLY TWD) for the awp ui framework
- Lzyps Fork Creator For Upkeep 

disclaimer:

**we do not condone the use of malicious Roblox scripts or abuse of Roblox executors.
This project is provided for entertainment and educational purposes only.
Use at your own risk. <3**

How to update DYNs signer made with love: hi, heres a tutorial on how to update dyn's bytecode signer for any version, the tutorial uses the idb for latest
some notes:
- i forgot to mention that the function containing the key rva is the verify function ie `Native::Verify`
- change the minimum payload to 256 else scripts below 1024 bytes wont be signed and thus wont load since roblox changes its minimum for signing check based on a runtime flag thus better to assume maximum possible minimum and sign
idapython script you will need:

import ida_bytes
def dump(rva, size, path):
    data = ida_bytes.get_bytes(rva, size)
    open(path, "wb").write(data)
    print(f"written {size} bytes to {path}")
dump(0x6914A80, 0x10, "signature_constant.bin") # 1 x 16 bytes = 16 bytes
dump(0x6924A90, 0x10000, "signature_linear_b.bin") # 4096 x 16 bytes = 64 kilobytes
dump(0x6914A90, 0x10000, "signature_linear_a.bin") # 4096 x 16 bytes = 64 kilobytes
dump(0x6934A90, 0x3c0000, "signature_quadratic.bin") # 245760 x 16 bytes = approx 3.75 megabytes

happy skidding!
