all: test_system.dll test_transitive.dll partial/test_system.dll test_input.dll test_input.exe

test_system.dll: test_system.c test_system.def
	$(CC) $(CFLAGS) -shared -o $@ $^

libtest_system.a: test_system_import.def
	$(DLLTOOL) -d $< -l $@

test_transitive.dll: test_transitive.c test_transitive.def
	$(CC) $(CFLAGS) -shared -o $@ $^

libtest_transitive.a: test_transitive_import.def
	$(DLLTOOL) -d $< -l $@

partial/test_system.dll: partial/test_system.c partial/test_system.def
	$(CC) $(CFLAGS) -shared -o $@ $^

test_input.dll: test_input.c libtest_system.a libtest_transitive.a
	$(CC) $(CFLAGS) -shared -o $@ test_input.c -L. -ltest_system -ltest_transitive

test_input.exe: test_input_exe.c libtest_system.a libtest_transitive.a
	$(CC) $(CFLAGS) -o $@ test_input_exe.c -L. -ltest_system -ltest_transitive

clean:
	rm -f *.dll *.a partial/*.dll
