// Build with scripts/build-layout-fixture.ps1 in an x64 MSVC developer shell.
enum CallbackResult { CallbackAccept = 1, CallbackReject = 2 };
struct CallbackTable {
    CallbackResult (*query)(void*);
    void (*(*exchange)(void (*)(unsigned long, volatile long*)))(unsigned long, volatile long*);
};

#pragma pack(push, 1)
struct PackedRecord {
    unsigned char tag;
    unsigned __int64 value;
};
#pragma pack(pop)

struct __declspec(align(16)) AlignedEntry { void* next; };
struct CacheLines {
    unsigned long first;
    __declspec(align(64)) unsigned long second;
    __declspec(align(64)) unsigned long third;
};

union ForwardValue;
struct ForwardPointer { const ForwardValue* pointer; };
union ForwardValue { unsigned __int64 value; };
struct QualifiedRecord {
    volatile ForwardValue value;
    unsigned long after;
};
struct ArrayRecord { ForwardValue values[1]; };

struct BitRecord {
    unsigned long first : 3;
    unsigned long : 2;
    unsigned long second : 6;
    unsigned long third : 2;
    unsigned char after;
};
struct AnonymousEnumRecord {
    enum { AnonymousAccept = 1, AnonymousReject = 2 } result;
};
enum WideEnum : unsigned __int64 { WideValue = 0xffffffffffffffffULL };
struct WideEnumRecord { WideEnum value; };
enum SignedEnum { SignedMinusOne = -1, SignedMinusTwo = -2, SignedMinus128 = -128 };
struct SignedEnumRecord { SignedEnum value; };
struct Qualifiers { const volatile unsigned long* both; unsigned long* const fixed; };

// Force complete CodeView type records into the linked PDB.
CallbackTable callback_table;
PackedRecord packed_record;
AlignedEntry aligned_entry;
CacheLines cache_lines;
ForwardPointer forward_pointer;
QualifiedRecord qualified_record;
ArrayRecord array_record;
BitRecord bit_record;
AnonymousEnumRecord anonymous_record;
WideEnumRecord wide_record;
SignedEnumRecord signed_record;
Qualifiers qualifiers = {0, 0};
