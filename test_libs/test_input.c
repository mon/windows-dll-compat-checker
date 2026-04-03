__declspec(dllimport) void Foo(void);
__declspec(dllimport) void Bar(void);
__declspec(dllimport) void Baz(void);
__declspec(dllimport) void TransFoo(void);

__declspec(dllexport) void TestEntry(void) {
    Foo();
    Bar();
    Baz();
    TransFoo();
}
