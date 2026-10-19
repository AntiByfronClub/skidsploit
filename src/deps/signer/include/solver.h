#pragma once

#include <array>
#include <cstdint>
#include <filesystem>
#include <vector>

namespace signing::detail
{
	struct bits
	{
		uint64_t low = 0;
		uint64_t high = 0;

		bool test(int index) const;
		void set(int index);
		int top() const;
		bool empty() const;
		bits& operator^=(bits other);
		friend bits operator^(bits left, bits right);
		bool operator==(const bits&) const = default;
	};

	class basis
	{
	public:
		std::array<bits, 128> rows{};
		int count = 0;

		void insert(bits value);
		bits reduce(bits value) const;
		std::vector<bits> kernel() const;
	};

	class solver
	{
	public:
		solver();
		bits evaluate(bits value) const;
		bits solve(bits target) const;

	private:
		struct layer
		{
			basis previous;
			std::array<bits, 96> images{};
			std::array<bits, 96> directions{};
		};

		void finalize_init();
		bits polynomial(bits value) const;
		bits byte_term(int index, uint8_t value) const;
		void prepare();

		bits m_constant;
		std::vector<bits> m_linear_a;
		std::vector<bits> m_linear_b;
		std::vector<bits> m_table;
		std::array<bits, 128> m_linear{};
		std::vector<bits> m_quadratic;
		std::vector<layer> m_layers;
	};
}