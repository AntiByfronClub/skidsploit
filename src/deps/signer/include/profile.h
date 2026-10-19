#pragma once

#include <array>
#include <cstddef>
#include <cstdint>

namespace signing::profile
{
	inline constexpr char version[] = "0.742.0.7421053"; /* my ex sending late night texts cuz she dont know how to let go go go */
	inline constexpr uint32_t key = 0x74171870;
	inline constexpr std::array<uint8_t, 4> marker{ 0x32, 0xc4, 0x6a, 1 };
	inline constexpr size_t minimum_payload = 1024;
	inline constexpr size_t maximum_payload = 64 * 1024 * 1024; /* fuck roblox. */
	inline constexpr size_t trailer_size = 24;

	namespace native
	{
		inline constexpr uintptr_t verify = 0x4239ae0;
		inline constexpr uintptr_t destroy_string = 0x722920;
		inline constexpr uintptr_t key = 0x6454adc;
		inline constexpr std::array<uint8_t, 18> load_entry{ 0x40, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x81, 0xec, 0x40, 0x01, 0x00 }; /* push rbx end->sub rsp, 0x140 */
	}
}