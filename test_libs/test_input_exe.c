__declspec(dllimport) void Foo(void);
__declspec(dllimport) void Bar(void);
__declspec(dllimport) void Baz(void);
__declspec(dllimport) void TransFoo(void);

int main(void) {
    Foo();
    Bar();
    Baz();
    TransFoo();
    return 0;
}
