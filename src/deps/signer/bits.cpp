#include "include/solver.h"
#include <bit>

namespace signing::detail
{
	bool bits::test(int index) const
	{
		return ((index < 64 ? low : high) >> (index & 63)) & 1;
	}

	void bits::set(int index)
	{
		(index < 64 ? low : high) |= uint64_t(1) << (index & 63);
	}

	int bits::top() const
	{
		return high ? 127 - std::countl_zero(high) : 63 - std::countl_zero(low);
	}

	bool bits::empty() const
	{
		return !(low | high);
	}

	bits& bits::operator^=(bits other)
	{
		low ^= other.low;
		high ^= other.high;
		return *this;
	}

	bits operator^(bits left, bits right)
	{
		return left ^= right;
	}

	void basis::insert(bits value)
	{
		while (!value.empty())
		{
			const int pivot = value.top();
			if (rows[pivot].empty())
			{
				rows[pivot] = value;
				++count;
				return;
			}
			value ^= rows[pivot];
		}
	}

	bits basis::reduce(bits value) const
	{
		for (int pivot = 127; pivot >= 0; --pivot)
		{
			if (value.test(pivot))
				value ^= rows[pivot];
		}
		return value;
	}

	std::vector<bits> basis::kernel() const
	{
		std::vector<bits> result;
		for (int free = 0; free < 128; ++free)
		{
			if (!rows[free].empty())
				continue;

			bits value;
			value.set(free);
			for (int pivot = 0; pivot < 128; ++pivot)
			{
				const bits& row = rows[pivot];
				if ((std::popcount(row.low & value.low) + std::popcount(row.high & value.high)) & 1)
					value.set(pivot);
			}
			result.push_back(value);
		}
		return result;
	}
}