; ModuleID = 'branch_instr_test.c'
source_filename = "branch_instr_test.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

@.str = private unnamed_addr constant [15 x i8] c"hello, world.\0A\00", align 1, !dbg !0
@stderr = external global ptr, align 8
@.str.1 = private unnamed_addr constant [18 x i8] c"Usage: %s <name>\0A\00", align 1, !dbg !7
@.str.2 = private unnamed_addr constant [13 x i8] c"Counter: %d\0A\00", align 1, !dbg !12
@.str.3 = private unnamed_addr constant [19 x i8] c"You entered zero.\0A\00", align 1, !dbg !17
@.str.4 = private unnamed_addr constant [18 x i8] c"You entered one.\0A\00", align 1, !dbg !22
@.str.5 = private unnamed_addr constant [43 x i8] c"You entered a number greater than one: %d\0A\00", align 1, !dbg !24

; Function Attrs: noinline nounwind optnone uwtable
define dso_local i32 @main(i32 noundef %0, ptr noundef %1) #0 !dbg !39 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 8
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store i32 0, ptr %3, align 4
  store i32 %0, ptr %4, align 4
    #dbg_declare(ptr %4, !46, !DIExpression(), !47)
  store ptr %1, ptr %5, align 8
    #dbg_declare(ptr %5, !48, !DIExpression(), !49)
  %8 = call i32 (ptr, ...) @printf(ptr noundef @.str), !dbg !50
  %9 = load i32, ptr %4, align 4, !dbg !51
  %10 = icmp ne i32 2, %9, !dbg !53
  br i1 %10, label %11, label %17, !dbg !54

11:                                               ; preds = %2
  %12 = load ptr, ptr @stderr, align 8, !dbg !55
  %13 = load ptr, ptr %5, align 8, !dbg !57
  %14 = getelementptr inbounds ptr, ptr %13, i64 0, !dbg !57
  %15 = load ptr, ptr %14, align 8, !dbg !57
  %16 = call i32 (ptr, ptr, ...) @fprintf(ptr noundef %12, ptr noundef @.str.1, ptr noundef %15) #4, !dbg !58
  store i32 1, ptr %3, align 4, !dbg !59
  br label %55, !dbg !59

17:                                               ; preds = %2
    #dbg_declare(ptr %6, !60, !DIExpression(), !61)
  %18 = load ptr, ptr %5, align 8, !dbg !62
  %19 = getelementptr inbounds ptr, ptr %18, i64 1, !dbg !62
  %20 = load ptr, ptr %19, align 8, !dbg !62
  %21 = call i32 @atoi(ptr noundef %20) #5, !dbg !63
  store i32 %21, ptr %6, align 4, !dbg !61
    #dbg_declare(ptr %7, !64, !DIExpression(), !65)
  store i32 0, ptr %7, align 4, !dbg !65
  br label %22, !dbg !66

22:                                               ; preds = %40, %17
  %23 = load i32, ptr %7, align 4, !dbg !67
  %24 = load i32, ptr %6, align 4, !dbg !68
  %25 = load i32, ptr %7, align 4, !dbg !69
  %26 = sub nsw i32 %24, %25, !dbg !70
  %27 = mul nsw i32 %23, %26, !dbg !71
  %28 = load i32, ptr %7, align 4, !dbg !72
  %29 = mul nsw i32 %28, 2, !dbg !73
  %30 = icmp sle i32 %27, %29, !dbg !74
  br i1 %30, label %31, label %38, !dbg !75

31:                                               ; preds = %22
  %32 = load i32, ptr %7, align 4, !dbg !76
  %33 = load i32, ptr %6, align 4, !dbg !77
  %34 = load i32, ptr %7, align 4, !dbg !78
  %35 = sub nsw i32 %33, %34, !dbg !79
  %36 = mul nsw i32 %32, %35, !dbg !80
  %37 = icmp sge i32 %36, 0, !dbg !81
  br label %38

38:                                               ; preds = %31, %22
  %39 = phi i1 [ false, %22 ], [ %37, %31 ], !dbg !82
  br i1 %39, label %40, label %45, !dbg !66

40:                                               ; preds = %38
  %41 = load i32, ptr %7, align 4, !dbg !83
  %42 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %41), !dbg !85
  %43 = load i32, ptr %7, align 4, !dbg !86
  %44 = add nsw i32 %43, 1, !dbg !86
  store i32 %44, ptr %7, align 4, !dbg !86
  br label %22, !dbg !66, !llvm.loop !87

45:                                               ; preds = %38
  %46 = load i32, ptr %6, align 4, !dbg !90
  switch i32 %46, label %51 [
    i32 0, label %47
    i32 1, label %49
  ], !dbg !91

47:                                               ; preds = %45
  %48 = call i32 (ptr, ...) @printf(ptr noundef @.str.3), !dbg !92
  store i32 0, ptr %3, align 4, !dbg !94
  br label %55, !dbg !94

49:                                               ; preds = %45
  %50 = call i32 (ptr, ...) @printf(ptr noundef @.str.4), !dbg !95
  br label %54, !dbg !96

51:                                               ; preds = %45
  %52 = load i32, ptr %6, align 4, !dbg !97
  %53 = call i32 (ptr, ...) @printf(ptr noundef @.str.5, i32 noundef %52), !dbg !98
  br label %54, !dbg !99

54:                                               ; preds = %51, %49
  store i32 0, ptr %3, align 4, !dbg !100
  br label %55, !dbg !100

55:                                               ; preds = %54, %47, %11
  %56 = load i32, ptr %3, align 4, !dbg !101
  ret i32 %56, !dbg !101
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind
declare i32 @fprintf(ptr noundef, ptr noundef, ...) #2

; Function Attrs: nounwind willreturn memory(read)
declare i32 @atoi(ptr noundef) #3

attributes #0 = { noinline nounwind optnone uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #2 = { nounwind "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #3 = { nounwind willreturn memory(read) "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #4 = { nounwind }
attributes #5 = { nounwind willreturn memory(read) }

!llvm.dbg.cu = !{!29}
!llvm.module.flags = !{!31, !32, !33, !34, !35, !36, !37}
!llvm.ident = !{!38}

!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(scope: null, file: !2, line: 5, type: !3, isLocal: true, isDefinition: true)
!2 = !DIFile(filename: "branch_instr_test.c", directory: "/struct_fuzz/func_stack_pass/demo_progs", checksumkind: CSK_MD5, checksum: "a2088a06669036973248028f86991943")
!3 = !DICompositeType(tag: DW_TAG_array_type, baseType: !4, size: 120, elements: !5)
!4 = !DIBasicType(name: "char", size: 8, encoding: DW_ATE_signed_char)
!5 = !{!6}
!6 = !DISubrange(count: 15)
!7 = !DIGlobalVariableExpression(var: !8, expr: !DIExpression())
!8 = distinct !DIGlobalVariable(scope: null, file: !2, line: 8, type: !9, isLocal: true, isDefinition: true)
!9 = !DICompositeType(tag: DW_TAG_array_type, baseType: !4, size: 144, elements: !10)
!10 = !{!11}
!11 = !DISubrange(count: 18)
!12 = !DIGlobalVariableExpression(var: !13, expr: !DIExpression())
!13 = distinct !DIGlobalVariable(scope: null, file: !2, line: 15, type: !14, isLocal: true, isDefinition: true)
!14 = !DICompositeType(tag: DW_TAG_array_type, baseType: !4, size: 104, elements: !15)
!15 = !{!16}
!16 = !DISubrange(count: 13)
!17 = !DIGlobalVariableExpression(var: !18, expr: !DIExpression())
!18 = distinct !DIGlobalVariable(scope: null, file: !2, line: 21, type: !19, isLocal: true, isDefinition: true)
!19 = !DICompositeType(tag: DW_TAG_array_type, baseType: !4, size: 152, elements: !20)
!20 = !{!21}
!21 = !DISubrange(count: 19)
!22 = !DIGlobalVariableExpression(var: !23, expr: !DIExpression())
!23 = distinct !DIGlobalVariable(scope: null, file: !2, line: 24, type: !9, isLocal: true, isDefinition: true)
!24 = !DIGlobalVariableExpression(var: !25, expr: !DIExpression())
!25 = distinct !DIGlobalVariable(scope: null, file: !2, line: 27, type: !26, isLocal: true, isDefinition: true)
!26 = !DICompositeType(tag: DW_TAG_array_type, baseType: !4, size: 344, elements: !27)
!27 = !{!28}
!28 = !DISubrange(count: 43)
!29 = distinct !DICompileUnit(language: DW_LANG_C11, file: !2, producer: "Ubuntu clang version 19.1.7 (++20250114103332+cd708029e0b2-1~exp1~20250114103446.78)", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, globals: !30, splitDebugInlining: false, nameTableKind: None)
!30 = !{!0, !7, !12, !17, !22, !24}
!31 = !{i32 7, !"Dwarf Version", i32 5}
!32 = !{i32 2, !"Debug Info Version", i32 3}
!33 = !{i32 1, !"wchar_size", i32 4}
!34 = !{i32 8, !"PIC Level", i32 2}
!35 = !{i32 7, !"PIE Level", i32 2}
!36 = !{i32 7, !"uwtable", i32 2}
!37 = !{i32 7, !"frame-pointer", i32 2}
!38 = !{!"Ubuntu clang version 19.1.7 (++20250114103332+cd708029e0b2-1~exp1~20250114103446.78)"}
!39 = distinct !DISubprogram(name: "main", scope: !2, file: !2, line: 4, type: !40, scopeLine: 4, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !29, retainedNodes: !45)
!40 = !DISubroutineType(types: !41)
!41 = !{!42, !42, !43}
!42 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!43 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !44, size: 64)
!44 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !4, size: 64)
!45 = !{}
!46 = !DILocalVariable(name: "argc", arg: 1, scope: !39, file: !2, line: 4, type: !42)
!47 = !DILocation(line: 4, column: 14, scope: !39)
!48 = !DILocalVariable(name: "argv", arg: 2, scope: !39, file: !2, line: 4, type: !43)
!49 = !DILocation(line: 4, column: 26, scope: !39)
!50 = !DILocation(line: 5, column: 3, scope: !39)
!51 = !DILocation(line: 7, column: 12, scope: !52)
!52 = distinct !DILexicalBlock(scope: !39, file: !2, line: 7, column: 7)
!53 = !DILocation(line: 7, column: 9, scope: !52)
!54 = !DILocation(line: 7, column: 7, scope: !39)
!55 = !DILocation(line: 8, column: 13, scope: !56)
!56 = distinct !DILexicalBlock(scope: !52, file: !2, line: 7, column: 18)
!57 = !DILocation(line: 8, column: 43, scope: !56)
!58 = !DILocation(line: 8, column: 5, scope: !56)
!59 = !DILocation(line: 9, column: 5, scope: !56)
!60 = !DILocalVariable(name: "num", scope: !39, file: !2, line: 12, type: !42)
!61 = !DILocation(line: 12, column: 7, scope: !39)
!62 = !DILocation(line: 12, column: 18, scope: !39)
!63 = !DILocation(line: 12, column: 13, scope: !39)
!64 = !DILocalVariable(name: "i", scope: !39, file: !2, line: 13, type: !42)
!65 = !DILocation(line: 13, column: 7, scope: !39)
!66 = !DILocation(line: 14, column: 3, scope: !39)
!67 = !DILocation(line: 14, column: 11, scope: !39)
!68 = !DILocation(line: 14, column: 16, scope: !39)
!69 = !DILocation(line: 14, column: 22, scope: !39)
!70 = !DILocation(line: 14, column: 20, scope: !39)
!71 = !DILocation(line: 14, column: 13, scope: !39)
!72 = !DILocation(line: 14, column: 29, scope: !39)
!73 = !DILocation(line: 14, column: 31, scope: !39)
!74 = !DILocation(line: 14, column: 26, scope: !39)
!75 = !DILocation(line: 14, column: 35, scope: !39)
!76 = !DILocation(line: 14, column: 38, scope: !39)
!77 = !DILocation(line: 14, column: 43, scope: !39)
!78 = !DILocation(line: 14, column: 49, scope: !39)
!79 = !DILocation(line: 14, column: 47, scope: !39)
!80 = !DILocation(line: 14, column: 40, scope: !39)
!81 = !DILocation(line: 14, column: 52, scope: !39)
!82 = !DILocation(line: 0, scope: !39)
!83 = !DILocation(line: 15, column: 29, scope: !84)
!84 = distinct !DILexicalBlock(scope: !39, file: !2, line: 14, column: 58)
!85 = !DILocation(line: 15, column: 5, scope: !84)
!86 = !DILocation(line: 16, column: 6, scope: !84)
!87 = distinct !{!87, !66, !88, !89}
!88 = !DILocation(line: 17, column: 3, scope: !39)
!89 = !{!"llvm.loop.mustprogress"}
!90 = !DILocation(line: 19, column: 11, scope: !39)
!91 = !DILocation(line: 19, column: 3, scope: !39)
!92 = !DILocation(line: 21, column: 5, scope: !93)
!93 = distinct !DILexicalBlock(scope: !39, file: !2, line: 19, column: 16)
!94 = !DILocation(line: 22, column: 5, scope: !93)
!95 = !DILocation(line: 24, column: 5, scope: !93)
!96 = !DILocation(line: 25, column: 5, scope: !93)
!97 = !DILocation(line: 27, column: 59, scope: !93)
!98 = !DILocation(line: 27, column: 5, scope: !93)
!99 = !DILocation(line: 28, column: 3, scope: !93)
!100 = !DILocation(line: 30, column: 3, scope: !39)
!101 = !DILocation(line: 31, column: 1, scope: !39)
