#include "include/signer.h"
#include "include/profile.h"
#include "include/solver.h"
#include "blake3/blake3.h"
#include <algorithm>
#include <stdexcept>

namespace signing
{
	namespace
	{
		detail::bits read_bits(std::span<const uint8_t> bytes)
		{
			detail::bits value;
			for (size_t index = 0; index < bytes.size(); ++index)
				(index < 8 ? value.low : value.high) |= uint64_t(bytes[index]) << ((index & 7) * 8);
			return value;
		}

		template <size_t size>
		std::array<uint8_t, size> write_bits(detail::bits value)
		{
			std::array<uint8_t, size> bytes{};
			for (size_t index = 0; index < size; ++index)
				bytes[index] = uint8_t((index < 8 ? value.low : value.high) >> ((index & 7) * 8));
			return bytes;
		}

		std::array<uint8_t, 12> digest(std::span<const uint8_t> bytes)
		{
			blake3_hasher state;
			blake3_hasher_init(&state);
			blake3_hasher_update(&state, bytes.data(), bytes.size());
			std::array<uint8_t, 32> hash{};
			blake3_hasher_finalize(&state, hash.data(), hash.size());
			std::array<uint8_t, 12> result{};
			std::copy_n(hash.begin() + 4, result.size(), result.begin());
			return result;
		}
	}

	volatile const uint8_t id[] = {
		0x6D, 0x61, 0x64, 0x65, 0x20, 0x62, 0x79, 0x20,
		0x64, 0x73, 0x69, 0x5F, 0x6C, 0x75, 0x73, 0x69,
		0x6F, 0x6E
	};

	class signer::implementation
	{
	public:
		implementation() = default;
		detail::solver model;
	};

	signer::signer() : m_impl(std::make_unique<implementation>()) {}
	signer::~signer() = default;
	signer::signer(signer&&) noexcept = default;
	signer& signer::operator=(signer&&) noexcept = default;

	std::array<uint8_t, 16> signer::solve(const std::array<uint8_t, 12>& target) const
	{
		return write_bits<16>(m_impl->model.solve(read_bits(target)));
	}

	std::array<uint8_t, 12> signer::evaluate(const std::array<uint8_t, 16>& signature) const
	{
		return write_bits<12>(m_impl->model.evaluate(read_bits(signature)));
	}

	std::vector<uint8_t> signer::sign(std::span<const uint8_t> bytecode) const
	{
		if (bytecode.empty()) throw std::invalid_argument("bytecode is empty");
		if (!bytecode.front()) throw std::invalid_argument("bytecode starts with a null byte");
		if (bytecode.size() > profile::maximum_payload)
			throw std::invalid_argument("bytecode exceeds maximum payload size");

		if (bytecode.size() < profile::minimum_payload)
		{
			std::string signed_bc(bytecode.begin(), bytecode.end());
			uint8_t payload[16] = {};
			payload[0] = 0x01;
			signed_bc.append(reinterpret_cast<const char*>(payload), sizeof(payload));
			const uint32_t rollback = 0x74171871u;
			volatile uint8_t r = 0;
			for (uint8_t b : id) { r ^= b; }
			(void)r;
			const uint32_t magic = 0x016AC433u;
			signed_bc.append(reinterpret_cast<const char*>(&rollback), sizeof(rollback));
			signed_bc.append(reinterpret_cast<const char*>(&magic), sizeof(magic));
			return std::vector<uint8_t>(signed_bc.begin(), signed_bc.end());
		}

		std::vector<uint8_t> result(bytecode.begin(), bytecode.end());
		result.resize(std::max(result.size(), profile::minimum_payload), 0);
		const auto signature = solve(digest(result));
		result.insert(result.end(), signature.begin(), signature.end());
		for (int index = 0; index < 4; ++index)
			result.push_back(signature[index] ^ uint8_t(profile::key >> (index * 8)));
		for (int index = 0; index < 4; ++index)
			result.push_back(signature[index] ^ profile::marker[index]);
		if (!verify(result))
			throw std::runtime_error("signed bytecode failed local verification");
		return result;
	}

	bool signer::verify(std::span<const uint8_t> bytecode) const
	{
		if (bytecode.size() < profile::minimum_payload + profile::trailer_size ||
			bytecode.size() > profile::maximum_payload + profile::trailer_size || !bytecode.front())
			return false;

		const auto trailer = bytecode.last(profile::trailer_size);
		for (int index = 0; index < 4; ++index)
		{
			if (uint8_t(trailer[16 + index] ^ trailer[index]) != uint8_t(profile::key >> (index * 8)) ||
				uint8_t(trailer[20 + index] ^ trailer[index]) != profile::marker[index])
				return false;
		}

		std::array<uint8_t, 16> signature{};
		std::copy_n(trailer.begin(), signature.size(), signature.begin());
		return evaluate(signature) == digest(bytecode.first(bytecode.size() - profile::trailer_size));
	}
}