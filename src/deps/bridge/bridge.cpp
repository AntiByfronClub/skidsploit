#include <Luau/Compiler.h>
#include <Luau/BytecodeBuilder.h>
#include <Luau/Bytecode.h>
#include <Luau/BytecodeUtils.h>
#include <signer/include/signer.h>

#define XXH_INLINE_ALL
#include <xxhash.h>
#include <zstd.h>

#include <algorithm>
#include <cctype>
#include <cstring>
#include <span>
#include <stdexcept>
#include <string>
#include <vector>

namespace {

class prepareEncoder : public Luau::BytecodeEncoder {
    void encode(uint32_t* data, size_t count) override {
        for (auto index = 0u; index < count;) {
            auto& opcode = *reinterpret_cast<uint8_t*>(data + index);
            index += Luau::getOpLength(LuauOpcode(opcode));
            opcode *= 227;
        }
    }
};

prepareEncoder encoder;
signing::signer signerInstance;

const char* const commonGlobals[] = {
    "Game", "Workspace", "game", "plugin", "script", "shared", "workspace", "_G", "_ENV", nullptr
};

bool isIdentChar(char c) {
    return std::isalnum(static_cast<unsigned char>(c)) || c == '_';
}

size_t longBracketEnd(const std::string& text, size_t openBracket) {
    size_t equals = openBracket + 1;
    while (equals < text.size() && text[equals] == '=') ++equals;
    if (equals >= text.size() || text[equals] != '[') return std::string::npos;

    const std::string close = "]" + std::string(equals - openBracket - 1, '=') + "]";
    const size_t found = text.find(close, equals + 1);
    if (found == std::string::npos) return std::string::npos;
    return found + close.size();
}

struct constRewriteResult {
    std::string source;
    bool failed = false;
};

constRewriteResult rewriteConstDeclarations(const std::string& source) {
    constRewriteResult result;
    std::string& output = result.source;
    output.reserve(source.size());

    std::vector<std::string> constNames;

    size_t index = 0;
    const size_t length = source.size();
    char lastSignificant = 0;
    std::string lastKeyword;

    auto operatorBefore = [&]() {
        if (lastSignificant == 0) return false;
        return std::strchr(".,([{=~<>+-*/%^#:", lastSignificant) != nullptr;
    };
    auto keywordBefore = [&]() {
        return lastKeyword == "and" || lastKeyword == "or" || lastKeyword == "not"
            || lastKeyword == "function" || lastKeyword == "return"
            || lastKeyword == "local" || lastKeyword == "until" || lastKeyword == "in";
    };

    while (index < length) {
        const char c = source[index];

        if (c == '-' && index + 1 < length && source[index + 1] == '-') {
            const size_t bracketEnd = longBracketEnd(source, index + 2);
            if (bracketEnd != std::string::npos) {
                output.append(source, index, bracketEnd - index);
                index = bracketEnd;
                continue;
            }

            size_t end = source.find('\n', index);
            if (end == std::string::npos) end = length;
            output.append(source, index, end - index);
            index = end;
            continue;
        }

        if (c == '[') {
            const size_t bracketEnd = longBracketEnd(source, index);
            if (bracketEnd != std::string::npos) {
                output.append(source, index, bracketEnd - index);
                index = bracketEnd;
                continue;
            }
        }

        if (c == '"' || c == '\'') {
            size_t end = index + 1;
            while (end < length) {
                if (source[end] == '\\') {
                    end += 2;
                    continue;
                }
                if (source[end] == c) {
                    ++end;
                    break;
                }
                ++end;
            }
            output.append(source, index, end - index);
            index = end;
            lastSignificant = c;
            lastKeyword.clear();
            continue;
        }

        if (isIdentChar(c)) {
            size_t end = index;
            while (end < length && isIdentChar(source[end])) ++end;
            const std::string word = source.substr(index, end - index);

            size_t after = end;
            while (after < length && (source[after] == ' ' || source[after] == '\t')) ++after;
            const bool nextIsName = after < length
                && (std::isalpha(static_cast<unsigned char>(source[after])) || source[after] == '_');

            if (word == "const" && nextIsName && !operatorBefore() && !keywordBefore()) {
                output += "local";
                lastKeyword = "local";

                size_t nameEnd = after;
                while (nameEnd < length && isIdentChar(source[nameEnd])) ++nameEnd;
                constNames.push_back(source.substr(after, nameEnd - after));
            } else {
                const bool isConst = std::find(constNames.begin(), constNames.end(), word) != constNames.end();
                if (isConst && !operatorBefore() && !keywordBefore()) {
                    size_t probe = end;
                    while (probe < length && (source[probe] == ' ' || source[probe] == '\t'
                        || source[probe] == '\r' || source[probe] == '\n')) {
                        ++probe;
                    }
                    const char next = probe < length ? source[probe] : 0;
                    const char next2 = probe + 1 < length ? source[probe + 1] : 0;

                    const bool plainAssign = next == '=' && next2 != '=';
                    const bool multiAssign = next == ',';
                    const bool compoundAssign = (next == '+' || next == '-' || next == '*'
                        || next == '/' || next == '%' || next == '^') && next2 == '=';
                    const bool concatAssign = next == '.' && next2 == '.';

                    if (plainAssign || multiAssign || compoundAssign || concatAssign) {
                        result.failed = true;
                        return result;
                    }
                }

                output += word;
                lastKeyword = word;
            }

            lastSignificant = output.back();
            index = end;
            continue;
        }

        if (c != ' ' && c != '\t' && c != '\r' && c != '\n') {
            lastSignificant = c;
            lastKeyword.clear();
        }
        output += c;
        ++index;
    }

    return result;
}

std::string compileSource(const std::string& source) {
    const constRewriteResult rewritten = rewriteConstDeclarations(source);
    if (rewritten.failed) {
        std::string message;
        message.push_back('\0');
        message += "Assigned expression must be a variable or a field";
        return message;
    }

    Luau::CompileOptions options;
    options.debugLevel = 1;
    options.optimizationLevel = 1;
    options.mutableGlobals = commonGlobals;
    options.vectorLib = "Vector3";
    options.vectorCtor = "new";
    options.vectorType = "Vector3";
    return Luau::compile(rewritten.source, options, {}, &encoder);
}

std::string compressRsB1(const std::string& bytecode) {
    const size_t dataSize = bytecode.size();
    const size_t maxSize = ZSTD_compressBound(dataSize);
    std::vector<char> buffer(maxSize + 8);

    const uint32_t dataSize32 = static_cast<uint32_t>(dataSize);
    std::memcpy(buffer.data(), "RSB1", 4);
    std::memcpy(buffer.data() + 4, &dataSize32, sizeof(dataSize32));

    const size_t compressedSize = ZSTD_compress(buffer.data() + 8, maxSize, bytecode.data(), dataSize, ZSTD_maxCLevel());
    if (ZSTD_isError(compressedSize)) return {};

    const size_t totalSize = compressedSize + 8;
    const uint32_t key = XXH32(buffer.data(), totalSize, 42);
    const uint8_t* keyBytes = reinterpret_cast<const uint8_t*>(&key);

    for (size_t index = 0; index < totalSize; ++index) {
        buffer[index] ^= static_cast<char>(keyBytes[index % 4] + static_cast<uint8_t>(index * 41));
    }

    return std::string(buffer.data(), totalSize);
}

void fillError(const std::string& message, uint8_t* error, int32_t errorCapacity, int32_t* errorLength) {
    if (errorLength != nullptr) *errorLength = 0;
    if (error == nullptr || errorCapacity <= 0 || errorLength == nullptr) return;
    size_t count = message.size();
    if (count > static_cast<size_t>(errorCapacity)) count = static_cast<size_t>(errorCapacity);
    std::memcpy(error, message.data(), count);
    *errorLength = static_cast<int32_t>(count);
}

}

extern "C" int32_t spExtract(
    const uint8_t* data,
    int32_t dataLength,
    uint8_t* out,
    int32_t outCapacity,
    int32_t* outLength) {

    if (outLength != nullptr) *outLength = 0;
    if (data == nullptr || dataLength <= 0 || out == nullptr) return 1;

    std::vector<char> buffer(static_cast<size_t>(dataLength));
    std::memcpy(buffer.data(), data, static_cast<size_t>(dataLength));

    if (buffer.size() >= 8) {
        static const char signBytes[4] = {'R', 'S', 'B', '1'};

        std::vector<char> decoded(buffer);
        uint8_t keyBytes[4] = {};
        for (size_t index = 0; index < 4; ++index) {
            const int keystream = static_cast<int>(static_cast<uint8_t>(decoded[index]) ^ static_cast<uint8_t>(signBytes[index]))
                - static_cast<int>(index * 41);
            keyBytes[index] = static_cast<uint8_t>(keystream & 0xFF);
        }
        for (size_t index = 0; index < decoded.size(); ++index) {
            const uint8_t keystream = static_cast<uint8_t>(keyBytes[index % 4] + index * 41);
            decoded[index] ^= static_cast<char>(keystream);
        }

        if (std::memcmp(decoded.data(), signBytes, 4) == 0) {
            uint32_t rawSize = 0;
            std::memcpy(&rawSize, decoded.data() + 4, sizeof(rawSize));
            if (rawSize > 0 && rawSize <= 64u * 1024u * 1024u
                && static_cast<int64_t>(rawSize) <= static_cast<int64_t>(outCapacity)) {
                const size_t result = ZSTD_decompress(out, rawSize, decoded.data() + 8, decoded.size() - 8);
                if (!ZSTD_isError(result)) {
                    if (outLength != nullptr) *outLength = static_cast<int32_t>(result);
                    return 0;
                }
            }
        }
    }

    if (static_cast<int64_t>(dataLength) > static_cast<int64_t>(outCapacity)) return 3;
    std::memcpy(out, data, static_cast<size_t>(dataLength));
    if (outLength != nullptr) *outLength = dataLength;
    return 0;
}

extern "C" int32_t spPrepare(
    const char* source,
    int32_t sourceLength,
    int32_t wrapFunction,
    uint8_t* out,
    int32_t outCapacity,
    int32_t* outLength,
    uint8_t* error,
    int32_t errorCapacity,
    int32_t* errorLength) {

    if (outLength != nullptr) *outLength = 0;
    if (errorLength != nullptr) *errorLength = 0;

    if (source == nullptr || sourceLength < 0) {
        fillError("invalid source", error, errorCapacity, errorLength);
        return 1;
    }

    try {
        const std::string effective = wrapFunction
            ? "return function() " + std::string(source, static_cast<size_t>(sourceLength)) + " end"
            : std::string(source, static_cast<size_t>(sourceLength));

        const std::string bytecode = compileSource(effective);
        if (bytecode.empty() || bytecode[0] == '\0') {
            std::string message = bytecode;
            message.erase(std::remove(message.begin(), message.end(), '\0'), message.end());
            if (message.empty()) message = "compile failed";
            fillError(message, error, errorCapacity, errorLength);
            return 2;
        }

        std::vector<uint8_t> signedBytecode;
        try {
            signedBytecode = signerInstance.sign(std::span<const uint8_t>(
                reinterpret_cast<const uint8_t*>(bytecode.data()), bytecode.size()));
        } catch (const std::exception& exception) {
            fillError(exception.what(), error, errorCapacity, errorLength);
            return 3;
        }

        const std::string compressed = compressRsB1(std::string(signedBytecode.begin(), signedBytecode.end()));
        if (compressed.empty() || compressed[0] == '\0') {
            fillError("compress failed", error, errorCapacity, errorLength);
            return 4;
        }

        if (out == nullptr || static_cast<int32_t>(compressed.size()) > outCapacity) {
            fillError("output buffer too small", error, errorCapacity, errorLength);
            return 5;
        }

        std::memcpy(out, compressed.data(), compressed.size());
        if (outLength != nullptr) *outLength = static_cast<int32_t>(compressed.size());
        return 0;
    } catch (const std::exception& exception) {
        fillError(exception.what(), error, errorCapacity, errorLength);
        return 6;
    } catch (...) {
        fillError("unknown failure", error, errorCapacity, errorLength);
        return 7;
    }
}
