; ModuleID = 'draton'
source_filename = "draton"

@draton_safepoint_flag = global i32 0
@TypeDesc_draton_closure = constant { i32, i32, [1 x i32] } { i32 16, i32 1, [1 x i32] [i32 8] }
@str.0 = private unnamed_addr constant [32 x i8] c"hello from Draton Early Preview\00", align 1

declare i8* @malloc(i64)

declare void @free(i8*)

declare i32 @puts(i8*)

declare void @abort()

define void @draton_safepoint_slow() {
entry:
  ret void
}

define i8* @draton_gc_alloc(i64 %0, i16 %1) {
entry:
  %gc.raw = call i8* @malloc(i64 %0)
  ret i8* %gc.raw
}

define void @draton_gc_write_barrier(i8* %0, i8** %1, i8* %2) {
entry:
  ret void
}

define i8* @draton_alloc(i64 %0) {
entry:
  %alloc.ptr = call i8* @draton_gc_alloc(i64 %0, i16 0)
  ret i8* %alloc.ptr
}

define void @draton_dealloc(i8* %0) {
entry:
  call void @free(i8* %0)
  ret void
}

define void @draton_print({ i64, i8* } %0) {
entry:
  %str.ptr = extractvalue { i64, i8* } %0, 1
  %1 = call i32 @puts(i8* %str.ptr)
  ret void
}

declare { i64, i8* } @draton_str_slice({ i64, i8* }, i64, i64)

declare { i64, i8* } @draton_str_concat({ i64, i8* }, { i64, i8* })

declare i1 @draton_str_contains({ i64, i8* }, { i64, i8* })

declare i1 @draton_str_starts_with({ i64, i8* }, { i64, i8* })

declare i1 @draton_str_eq({ i64, i8* }, { i64, i8* })

declare { i64, i8* } @draton_str_replace({ i64, i8* }, { i64, i8* }, { i64, i8* })

declare { i64, i8* } @draton_int_to_string(i64)

declare { i64, i8* } @draton_ascii_char(i64)

declare { i64, i8* } @draton_read_file({ i64, i8* })

declare i64 @draton_string_parse_int({ i64, i8* })

declare i64 @draton_string_parse_int_radix({ i64, i8* }, i64)

declare double @draton_string_parse_float({ i64, i8* })

declare void @draton_set_cli_args(i32, i8**)

declare i64 @draton_cli_argc()

declare { i64, i8* } @draton_cli_arg(i64)

declare { i64, i8* } @draton_host_ast_dump({ i64, i8* })

declare { i64, i8* } @draton_host_type_dump({ i64, i8* })

define void @draton_panic({ i64, i8* } %0, { i64, i8* } %1, i64 %2) {
entry:
  call void @draton_print({ i64, i8* } %0)
  call void @abort()
  unreachable
}

declare void @__draton_std_io_eprintln({ i64, i8* })

declare { i64, i8* } @__draton_std_io_read_line()

declare { i64, i8* } @__draton_std_io_read_file({ i64, i8* })

declare i1 @__draton_std_io_write_file({ i64, i8* }, { i64, i8* })

declare i1 @__draton_std_io_append_file({ i64, i8* }, { i64, i8* })

declare i1 @__draton_std_io_file_exists({ i64, i8* })

define void @eprintln({ i64, i8* } %0) {
entry:
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  call void @__draton_std_io_eprintln({ i64, i8* } %s1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret void

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @read_line() {
entry:
  %call = call { i64, i8* } @__draton_std_io_read_line()
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @read_file({ i64, i8* } %0) {
entry:
  %path = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %path, align 8
  %path1 = load { i64, i8* }, { i64, i8* }* %path, align 8
  %call = call { i64, i8* } @__draton_std_io_read_file({ i64, i8* } %path1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define i1 @write_file({ i64, i8* } %0, { i64, i8* } %1) {
entry:
  %content = alloca { i64, i8* }, align 8
  %path = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %path, align 8
  store { i64, i8* } %1, { i64, i8* }* %content, align 8
  %path1 = load { i64, i8* }, { i64, i8* }* %path, align 8
  %content2 = load { i64, i8* }, { i64, i8* }* %content, align 8
  %call = call i1 @__draton_std_io_write_file({ i64, i8* } %path1, { i64, i8* } %content2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret i1 %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define i1 @append_file({ i64, i8* } %0, { i64, i8* } %1) {
entry:
  %content = alloca { i64, i8* }, align 8
  %path = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %path, align 8
  store { i64, i8* } %1, { i64, i8* }* %content, align 8
  %path1 = load { i64, i8* }, { i64, i8* }* %path, align 8
  %content2 = load { i64, i8* }, { i64, i8* }* %content, align 8
  %call = call i1 @__draton_std_io_append_file({ i64, i8* } %path1, { i64, i8* } %content2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret i1 %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define i1 @file_exists({ i64, i8* } %0) {
entry:
  %path = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %path, align 8
  %path1 = load { i64, i8* }, { i64, i8* }* %path, align 8
  %call = call i1 @__draton_std_io_file_exists({ i64, i8* } %path1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret i1 %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

declare { i64, { i64, i8* }* } @__draton_std_string_split({ i64, i8* }, { i64, i8* })

declare { i64, i8* } @__draton_std_string_trim({ i64, i8* })

declare { i64, i8* } @__draton_std_string_trim_start({ i64, i8* })

declare { i64, i8* } @__draton_std_string_trim_end({ i64, i8* })

declare { i64, i8* } @__draton_std_string_to_upper({ i64, i8* })

declare { i64, i8* } @__draton_std_string_to_lower({ i64, i8* })

declare { i1, i64 } @__draton_std_string_parse_int({ i64, i8* })

declare { i1, double } @__draton_std_string_parse_float({ i64, i8* })

declare { i64, i8* } @__draton_std_string_join({ i64, { i64, i8* }* }, { i64, i8* })

declare { i64, i8* } @__draton_std_string_repeat({ i64, i8* }, i64)

declare i64 @__draton_std_string_index_of({ i64, i8* }, { i64, i8* })

declare i1 @__draton_std_string_ends_with({ i64, i8* }, { i64, i8* })

declare i1 @__draton_std_string_contains({ i64, i8* }, { i64, i8* })

declare i1 @__draton_std_string_starts_with({ i64, i8* }, { i64, i8* })

declare { i64, i8* } @__draton_std_string_replace({ i64, i8* }, { i64, i8* }, { i64, i8* })

declare { i64, i8* } @__draton_std_string_slice({ i64, i8* }, i64, i64)

declare { i64, i8* } @__draton_std_int_to_string(i64)

declare { i64, i8* } @__draton_std_float_to_string(double)

define { i64, { i64, i8* }* } @split({ i64, i8* } %0, { i64, i8* } %1) {
entry:
  %sep = alloca { i64, i8* }, align 8
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  store { i64, i8* } %1, { i64, i8* }* %sep, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %sep2 = load { i64, i8* }, { i64, i8* }* %sep, align 8
  %call = call { i64, { i64, i8* }* } @__draton_std_string_split({ i64, i8* } %s1, { i64, i8* } %sep2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, { i64, i8* }* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @trim({ i64, i8* } %0) {
entry:
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %call = call { i64, i8* } @__draton_std_string_trim({ i64, i8* } %s1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @trim_start({ i64, i8* } %0) {
entry:
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %call = call { i64, i8* } @__draton_std_string_trim_start({ i64, i8* } %s1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @trim_end({ i64, i8* } %0) {
entry:
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %call = call { i64, i8* } @__draton_std_string_trim_end({ i64, i8* } %s1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @to_upper({ i64, i8* } %0) {
entry:
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %call = call { i64, i8* } @__draton_std_string_to_upper({ i64, i8* } %s1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @to_lower({ i64, i8* } %0) {
entry:
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %call = call { i64, i8* } @__draton_std_string_to_lower({ i64, i8* } %s1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i1, i64 } @parse_int({ i64, i8* } %0) {
entry:
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %call = call { i1, i64 } @__draton_std_string_parse_int({ i64, i8* } %s1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i1, i64 } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i1, double } @parse_float({ i64, i8* } %0) {
entry:
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %call = call { i1, double } @__draton_std_string_parse_float({ i64, i8* } %s1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i1, double } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @join({ i64, { i64, i8* }* } %0, { i64, i8* } %1) {
entry:
  %sep = alloca { i64, i8* }, align 8
  %parts = alloca { i64, { i64, i8* }* }, align 8
  store { i64, { i64, i8* }* } %0, { i64, { i64, i8* }* }* %parts, align 8
  store { i64, i8* } %1, { i64, i8* }* %sep, align 8
  %parts1 = load { i64, { i64, i8* }* }, { i64, { i64, i8* }* }* %parts, align 8
  %sep2 = load { i64, i8* }, { i64, i8* }* %sep, align 8
  %call = call { i64, i8* } @__draton_std_string_join({ i64, { i64, i8* }* } %parts1, { i64, i8* } %sep2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @repeat({ i64, i8* } %0, i64 %1) {
entry:
  %n = alloca i64, align 8
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  store i64 %1, i64* %n, align 4
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %n2 = load i64, i64* %n, align 4
  %call = call { i64, i8* } @__draton_std_string_repeat({ i64, i8* } %s1, i64 %n2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define i64 @index_of({ i64, i8* } %0, { i64, i8* } %1) {
entry:
  %sub = alloca { i64, i8* }, align 8
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  store { i64, i8* } %1, { i64, i8* }* %sub, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %sub2 = load { i64, i8* }, { i64, i8* }* %sub, align 8
  %call = call i64 @__draton_std_string_index_of({ i64, i8* } %s1, { i64, i8* } %sub2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret i64 %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define i1 @ends_with({ i64, i8* } %0, { i64, i8* } %1) {
entry:
  %suffix = alloca { i64, i8* }, align 8
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  store { i64, i8* } %1, { i64, i8* }* %suffix, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %suffix2 = load { i64, i8* }, { i64, i8* }* %suffix, align 8
  %call = call i1 @__draton_std_string_ends_with({ i64, i8* } %s1, { i64, i8* } %suffix2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret i1 %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define i1 @contains({ i64, i8* } %0, { i64, i8* } %1) {
entry:
  %sub = alloca { i64, i8* }, align 8
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  store { i64, i8* } %1, { i64, i8* }* %sub, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %sub2 = load { i64, i8* }, { i64, i8* }* %sub, align 8
  %call = call i1 @__draton_std_string_contains({ i64, i8* } %s1, { i64, i8* } %sub2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret i1 %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define i1 @starts_with({ i64, i8* } %0, { i64, i8* } %1) {
entry:
  %prefix = alloca { i64, i8* }, align 8
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  store { i64, i8* } %1, { i64, i8* }* %prefix, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %prefix2 = load { i64, i8* }, { i64, i8* }* %prefix, align 8
  %call = call i1 @__draton_std_string_starts_with({ i64, i8* } %s1, { i64, i8* } %prefix2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret i1 %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @replace({ i64, i8* } %0, { i64, i8* } %1, { i64, i8* } %2) {
entry:
  %replacement = alloca { i64, i8* }, align 8
  %needle = alloca { i64, i8* }, align 8
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  store { i64, i8* } %1, { i64, i8* }* %needle, align 8
  store { i64, i8* } %2, { i64, i8* }* %replacement, align 8
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %needle2 = load { i64, i8* }, { i64, i8* }* %needle, align 8
  %replacement3 = load { i64, i8* }, { i64, i8* }* %replacement, align 8
  %call = call { i64, i8* } @__draton_std_string_replace({ i64, i8* } %s1, { i64, i8* } %needle2, { i64, i8* } %replacement3)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @slice({ i64, i8* } %0, i64 %1, i64 %2) {
entry:
  %end = alloca i64, align 8
  %start = alloca i64, align 8
  %s = alloca { i64, i8* }, align 8
  store { i64, i8* } %0, { i64, i8* }* %s, align 8
  store i64 %1, i64* %start, align 4
  store i64 %2, i64* %end, align 4
  %s1 = load { i64, i8* }, { i64, i8* }* %s, align 8
  %start2 = load i64, i64* %start, align 4
  %end3 = load i64, i64* %end, align 4
  %call = call { i64, i8* } @__draton_std_string_slice({ i64, i8* } %s1, i64 %start2, i64 %end3)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @int_to_string(i64 %0) {
entry:
  %n = alloca i64, align 8
  store i64 %0, i64* %n, align 4
  %n1 = load i64, i64* %n, align 4
  %call = call { i64, i8* } @__draton_std_int_to_string(i64 %n1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i8* } @float_to_string(double %0) {
entry:
  %f = alloca double, align 8
  store double %0, double* %f, align 8
  %f1 = load double, double* %f, align 8
  %call = call { i64, i8* } @__draton_std_float_to_string(double %f1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i8* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

declare double @__draton_std_math_sqrt(double)

declare double @__draton_std_math_pow(double, double)

declare double @__draton_std_math_abs(double)

declare double @__draton_std_math_floor(double)

declare double @__draton_std_math_ceil(double)

declare double @__draton_std_math_round(double)

declare double @__draton_std_math_sin(double)

declare double @__draton_std_math_cos(double)

declare double @__draton_std_math_tan(double)

declare double @__draton_std_math_log(double)

declare double @__draton_std_math_log2(double)

declare double @__draton_std_math_log10(double)

declare double @__draton_std_math_min(double, double)

declare double @__draton_std_math_max(double, double)

declare double @__draton_std_math_clamp(double, double, double)

declare double @__draton_std_math_pi()

declare double @__draton_std_math_e()

declare { i1, i64 } @__draton_std_math_checked_add(i64, i64)

declare { i1, i64 } @__draton_std_math_checked_sub(i64, i64)

declare { i1, i64 } @__draton_std_math_checked_mul(i64, i64)

declare { i1, i64 } @__draton_std_math_checked_div(i64, i64)

define double @sqrt(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_sqrt(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @pow(double %0, double %1) {
entry:
  %exp = alloca double, align 8
  %base = alloca double, align 8
  store double %0, double* %base, align 8
  store double %1, double* %exp, align 8
  %base1 = load double, double* %base, align 8
  %exp2 = load double, double* %exp, align 8
  %call = call double @__draton_std_math_pow(double %base1, double %exp2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @abs(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_abs(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @floor(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_floor(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @ceil(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_ceil(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @round(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_round(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @sin(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_sin(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @cos(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_cos(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @tan(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_tan(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @log(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_log(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @log2(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_log2(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @log10(double %0) {
entry:
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  %x1 = load double, double* %x, align 8
  %call = call double @__draton_std_math_log10(double %x1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @min(double %0, double %1) {
entry:
  %b = alloca double, align 8
  %a = alloca double, align 8
  store double %0, double* %a, align 8
  store double %1, double* %b, align 8
  %a1 = load double, double* %a, align 8
  %b2 = load double, double* %b, align 8
  %call = call double @__draton_std_math_min(double %a1, double %b2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @max(double %0, double %1) {
entry:
  %b = alloca double, align 8
  %a = alloca double, align 8
  store double %0, double* %a, align 8
  store double %1, double* %b, align 8
  %a1 = load double, double* %a, align 8
  %b2 = load double, double* %b, align 8
  %call = call double @__draton_std_math_max(double %a1, double %b2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @clamp(double %0, double %1, double %2) {
entry:
  %hi = alloca double, align 8
  %lo = alloca double, align 8
  %x = alloca double, align 8
  store double %0, double* %x, align 8
  store double %1, double* %lo, align 8
  store double %2, double* %hi, align 8
  %x1 = load double, double* %x, align 8
  %lo2 = load double, double* %lo, align 8
  %hi3 = load double, double* %hi, align 8
  %call = call double @__draton_std_math_clamp(double %x1, double %lo2, double %hi3)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @pi() {
entry:
  %call = call double @__draton_std_math_pi()
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define double @e() {
entry:
  %call = call double @__draton_std_math_e()
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret double %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i1, i64 } @checked_add(i64 %0, i64 %1) {
entry:
  %b = alloca i64, align 8
  %a = alloca i64, align 8
  store i64 %0, i64* %a, align 4
  store i64 %1, i64* %b, align 4
  %a1 = load i64, i64* %a, align 4
  %b2 = load i64, i64* %b, align 4
  %call = call { i1, i64 } @__draton_std_math_checked_add(i64 %a1, i64 %b2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i1, i64 } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i1, i64 } @checked_sub(i64 %0, i64 %1) {
entry:
  %b = alloca i64, align 8
  %a = alloca i64, align 8
  store i64 %0, i64* %a, align 4
  store i64 %1, i64* %b, align 4
  %a1 = load i64, i64* %a, align 4
  %b2 = load i64, i64* %b, align 4
  %call = call { i1, i64 } @__draton_std_math_checked_sub(i64 %a1, i64 %b2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i1, i64 } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i1, i64 } @checked_mul(i64 %0, i64 %1) {
entry:
  %b = alloca i64, align 8
  %a = alloca i64, align 8
  store i64 %0, i64* %a, align 4
  store i64 %1, i64* %b, align 4
  %a1 = load i64, i64* %a, align 4
  %b2 = load i64, i64* %b, align 4
  %call = call { i1, i64 } @__draton_std_math_checked_mul(i64 %a1, i64 %b2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i1, i64 } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i1, i64 } @checked_div(i64 %0, i64 %1) {
entry:
  %b = alloca i64, align 8
  %a = alloca i64, align 8
  store i64 %0, i64* %a, align 4
  store i64 %1, i64* %b, align 4
  %a1 = load i64, i64* %a, align 4
  %b2 = load i64, i64* %b, align 4
  %call = call { i1, i64 } @__draton_std_math_checked_div(i64 %a1, i64 %b2)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i1, i64 } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

declare i64 @__draton_std_collections_sum({ i64, i64* })

declare i64 @__draton_std_collections_product({ i64, i64* })

declare { i64, i64* } @__draton_std_collections_reverse_int({ i64, i64* })

declare { i64, i64* } @__draton_std_collections_sort_int({ i64, i64* })

declare { i64, i64* } @__draton_std_collections_unique_int({ i64, i64* })

define i64 @sum({ i64, i64* } %0) {
entry:
  %arr = alloca { i64, i64* }, align 8
  store { i64, i64* } %0, { i64, i64* }* %arr, align 8
  %arr1 = load { i64, i64* }, { i64, i64* }* %arr, align 8
  %call = call i64 @__draton_std_collections_sum({ i64, i64* } %arr1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret i64 %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define i64 @product({ i64, i64* } %0) {
entry:
  %arr = alloca { i64, i64* }, align 8
  store { i64, i64* } %0, { i64, i64* }* %arr, align 8
  %arr1 = load { i64, i64* }, { i64, i64* }* %arr, align 8
  %call = call i64 @__draton_std_collections_product({ i64, i64* } %arr1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret i64 %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i64* } @reverse_int({ i64, i64* } %0) {
entry:
  %arr = alloca { i64, i64* }, align 8
  store { i64, i64* } %0, { i64, i64* }* %arr, align 8
  %arr1 = load { i64, i64* }, { i64, i64* }* %arr, align 8
  %call = call { i64, i64* } @__draton_std_collections_reverse_int({ i64, i64* } %arr1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i64* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i64* } @sort_int({ i64, i64* } %0) {
entry:
  %arr = alloca { i64, i64* }, align 8
  store { i64, i64* } %0, { i64, i64* }* %arr, align 8
  %arr1 = load { i64, i64* }, { i64, i64* }* %arr, align 8
  %call = call { i64, i64* } @__draton_std_collections_sort_int({ i64, i64* } %arr1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i64* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define { i64, i64* } @unique_int({ i64, i64* } %0) {
entry:
  %arr = alloca { i64, i64* }, align 8
  store { i64, i64* } %0, { i64, i64* }* %arr, align 8
  %arr1 = load { i64, i64* }, { i64, i64* }* %arr, align 8
  %call = call { i64, i64* } @__draton_std_collections_unique_int({ i64, i64* } %arr1)
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  ret { i64, i64* } %call

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont
}

define i64 @draton_user_main() {
entry:
  %call = call { i64, i8* } @hello()
  %safepoint.flag = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop = icmp ne i32 %safepoint.flag, 0
  br i1 %safepoint.need_stop, label %safepoint.slow, label %safepoint.cont

safepoint.cont:                                   ; preds = %safepoint.slow, %entry
  call void @draton_print({ i64, i8* } %call)
  %safepoint.flag3 = load i32, i32* @draton_safepoint_flag, align 4
  %safepoint.need_stop4 = icmp ne i32 %safepoint.flag3, 0
  br i1 %safepoint.need_stop4, label %safepoint.slow2, label %safepoint.cont1

safepoint.slow:                                   ; preds = %entry
  call void @draton_safepoint_slow()
  br label %safepoint.cont

safepoint.cont1:                                  ; preds = %safepoint.slow2, %safepoint.cont
  ret i64 0

safepoint.slow2:                                  ; preds = %safepoint.cont
  call void @draton_safepoint_slow()
  br label %safepoint.cont1
}

define { i64, i8* } @hello() {
entry:
  ret { i64, i8* } { i64 31, i8* getelementptr inbounds ([32 x i8], [32 x i8]* @str.0, i32 0, i32 0) }
}

define i64 @main(i32 %0, i8** %1) {
entry:
  call void @draton_set_cli_args(i32 %0, i8** %1)
  %drat.main = call i64 @draton_user_main()
  ret i64 %drat.main
}
