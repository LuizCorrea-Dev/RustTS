	.def	@feat.00;
	.scl	3;
	.type	0;
	.endef
	.globl	@feat.00
@feat.00 = 0
	.file	"ballistics_10k_benchmark.c"
	.def	engine_ballistics_step_ballistics_10k;
	.scl	2;
	.type	32;
	.endef
	.globl	__real@c11cf5c3                 # -- Begin function engine_ballistics_step_ballistics_10k
	.section	.rdata,"dr",discard,__real@c11cf5c3
	.p2align	2, 0x0
__real@c11cf5c3:
	.long	0xc11cf5c3                      # float -9.81000041
	.globl	__real@3f1ccccd
	.section	.rdata,"dr",discard,__real@3f1ccccd
	.p2align	2, 0x0
__real@3f1ccccd:
	.long	0x3f1ccccd                      # float 0.612500011
	.globl	__real@00000001
	.section	.rdata,"dr",discard,__real@00000001
	.p2align	2, 0x0
__real@00000001:
	.long	1                               # 0x1
	.globl	__real@bf266666
	.section	.rdata,"dr",discard,__real@bf266666
	.p2align	2, 0x0
__real@bf266666:
	.long	0xbf266666                      # float -0.649999976
	.text
	.globl	engine_ballistics_step_ballistics_10k
	.p2align	4
engine_ballistics_step_ballistics_10k:  # @engine_ballistics_step_ballistics_10k
.seh_proc engine_ballistics_step_ballistics_10k
# %bb.0:
	pushq	%rbp
	.seh_pushreg %rbp
	pushq	%r15
	.seh_pushreg %r15
	pushq	%r14
	.seh_pushreg %r14
	pushq	%r13
	.seh_pushreg %r13
	pushq	%r12
	.seh_pushreg %r12
	pushq	%rsi
	.seh_pushreg %rsi
	pushq	%rdi
	.seh_pushreg %rdi
	pushq	%rbx
	.seh_pushreg %rbx
	subq	$1464, %rsp                     # imm = 0x5B8
	.seh_stackalloc 1464
	leaq	128(%rsp), %rbp
	.seh_setframe %rbp, 128
	vmovaps	%xmm14, 1312(%rbp)              # 16-byte Spill
	.seh_savexmm %xmm14, 1440
	vmovaps	%xmm13, 1296(%rbp)              # 16-byte Spill
	.seh_savexmm %xmm13, 1424
	vmovaps	%xmm12, 1280(%rbp)              # 16-byte Spill
	.seh_savexmm %xmm12, 1408
	vmovaps	%xmm11, 1264(%rbp)              # 16-byte Spill
	.seh_savexmm %xmm11, 1392
	vmovaps	%xmm10, 1248(%rbp)              # 16-byte Spill
	.seh_savexmm %xmm10, 1376
	vmovaps	%xmm9, 1232(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm9, 1360
	vmovaps	%xmm8, 1216(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm8, 1344
	vmovaps	%xmm7, 1200(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm7, 1328
	vmovaps	%xmm6, 1184(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm6, 1312
	.seh_endprologue
	andq	$-64, %rsp
	movq	%rcx, %r11
	movq	(%rdx), %rsi
	cmpq	(%r8), %rsi
	jne	.LBB0_1
# %bb.2:
	movq	1440(%rbp), %rax
	vmovss	(%rax), %xmm0                   # xmm0 = mem[0],zero,zero,zero
	vxorps	%xmm1, %xmm1, %xmm1
	vucomiss	%xmm0, %xmm1
	jae	.LBB0_3
# %bb.4:
	movq	%rdx, %r10
	movq	8(%r9), %rcx
	movq	16(%r9), %rdi
	addq	$3, %rdi
	andq	$-4, %rdi
	subq	%rdi, %rcx
	movl	$0, %eax
	cmovaeq	%rcx, %rax
	movabsq	$-3689348814741910323, %rcx     # imm = 0xCCCCCCCCCCCCCCCD
	mulq	%rcx
	testq	%rsi, %rsi
	je	.LBB0_27
# %bb.5:
	movq	$0, 40(%rsp)                    # 8-byte Folded Spill
	movq	%r11, 128(%rsp)                 # 8-byte Spill
	movq	8(%r10), %r11
	movq	16(%r10), %rax
	movq	24(%r10), %rcx
	movq	32(%r10), %rbx
	movq	16(%r8), %r13
	movq	24(%r8), %r14
	movq	32(%r8), %r15
	movq	40(%r8), %r8
	shrq	$4, %rdx
	movq	%rdx, 176(%rsp)                 # 8-byte Spill
	movq	%r9, 192(%rsp)                  # 8-byte Spill
	movq	(%r9), %rdx
	movq	%rdi, 184(%rsp)                 # 8-byte Spill
	addq	%rdi, %rdx
	movq	%rdx, 168(%rsp)                 # 8-byte Spill
	vbroadcastss	%xmm0, %ymm1
	movl	$256, %r12d                     # imm = 0x100
	leaq	256(%rsp), %r9
	vmovss	__real@c11cf5c3(%rip), %xmm2    # xmm2 = [-9.81000041E+0,0.0E+0,0.0E+0,0.0E+0]
	vmovss	__real@3f1ccccd(%rip), %xmm3    # xmm3 = [6.12500011E-1,0.0E+0,0.0E+0,0.0E+0]
	vxorps	%xmm4, %xmm4, %xmm4
	vmovss	__real@bf266666(%rip), %xmm5    # xmm5 = [-6.49999976E-1,0.0E+0,0.0E+0,0.0E+0]
	vbroadcastss	__real@c11cf5c3(%rip), %ymm6 # ymm6 = [-9.81000041E+0,-9.81000041E+0,-9.81000041E+0,-9.81000041E+0,-9.81000041E+0,-9.81000041E+0,-9.81000041E+0,-9.81000041E+0]
	vbroadcastss	__real@3f1ccccd(%rip), %ymm7 # ymm7 = [6.12500011E-1,6.12500011E-1,6.12500011E-1,6.12500011E-1,6.12500011E-1,6.12500011E-1,6.12500011E-1,6.12500011E-1]
	vbroadcastss	__real@00000001(%rip), %ymm8 # ymm8 = [1,1,1,1,1,1,1,1]
	vxorps	%xmm9, %xmm9, %xmm9
	movq	%r13, 224(%rsp)                 # 8-byte Spill
	movq	%r8, 200(%rsp)                  # 8-byte Spill
	movq	%r14, 216(%rsp)                 # 8-byte Spill
	movq	%r15, 208(%rsp)                 # 8-byte Spill
	movq	%rax, 248(%rsp)                 # 8-byte Spill
	movq	%rcx, 240(%rsp)                 # 8-byte Spill
	movq	%rcx, %r10
	movq	%rbx, 232(%rsp)                 # 8-byte Spill
	movq	%rbx, %rdi
	movq	%rax, %rbx
	xorl	%edx, %edx
	xorl	%ecx, %ecx
	movq	$0, 136(%rsp)                   # 8-byte Folded Spill
	xorl	%eax, %eax
	movq	%rsi, 144(%rsp)                 # 8-byte Spill
	jmp	.LBB0_6
	.p2align	4
.LBB0_7:                                #   in Loop: Header=BB0_6 Depth=1
	movq	120(%rsp), %r11                 # 8-byte Reload
	movq	80(%rsp), %r9                   # 8-byte Reload
.LBB0_26:                               #   in Loop: Header=BB0_6 Depth=1
	movq	160(%rsp), %rax                 # 8-byte Reload
	addq	$256, %rax                      # imm = 0x100
	addq	$256, %r12                      # imm = 0x100
	incq	%rcx
	addq	$-256, %rdx
	addq	$1024, %rdi                     # imm = 0x400
	addq	$1024, %r10                     # imm = 0x400
	addq	$1024, %rbx                     # imm = 0x400
	addq	$1024, %r15                     # imm = 0x400
	addq	$1024, %r14                     # imm = 0x400
	addq	$1024, %r8                      # imm = 0x400
	addq	$1024, %r13                     # imm = 0x400
	addq	$-1024, %r9                     # imm = 0xFC00
	addq	$2048, %r11                     # imm = 0x800
	cmpq	%rsi, %rax
	jae	.LBB0_8
.LBB0_6:                                # =>This Loop Header: Depth=1
                                        #     Child Loop BB0_13 Depth 2
                                        #     Child Loop BB0_16 Depth 2
                                        #     Child Loop BB0_20 Depth 2
	movq	%r9, 80(%rsp)                   # 8-byte Spill
	movq	%r11, 120(%rsp)                 # 8-byte Spill
	cmpq	%r12, %rsi
	movq	%r12, %r9
	cmovbq	%rsi, %r9
	cmpq	%rax, %rsi
	movq	%rax, 160(%rsp)                 # 8-byte Spill
	jbe	.LBB0_7
# %bb.11:                               #   in Loop: Header=BB0_6 Depth=1
	movq	%rax, %rsi
	movq	%rdx, 64(%rsp)                  # 8-byte Spill
	movq	%rbx, 88(%rsp)                  # 8-byte Spill
	movq	%r15, 96(%rsp)                  # 8-byte Spill
	movq	%r14, 104(%rsp)                 # 8-byte Spill
	movq	%r8, 72(%rsp)                   # 8-byte Spill
	movq	%r13, 112(%rsp)                 # 8-byte Spill
	movq	%r12, 152(%rsp)                 # 8-byte Spill
	movq	%rcx, 48(%rsp)                  # 8-byte Spill
	movq	%rcx, %rax
	shlq	$8, %rax
	movq	%r9, 56(%rsp)                   # 8-byte Spill
	subq	%rax, %r9
	xorl	%eax, %eax
	movq	%rsi, %r12
	cmpq	$8, %r9
	jb	.LBB0_15
# %bb.12:                               #   in Loop: Header=BB0_6 Depth=1
	movq	56(%rsp), %rdx                  # 8-byte Reload
	movq	%rdx, %rax
	andq	$-8, %rax
	addq	64(%rsp), %rax                  # 8-byte Folded Reload
	movl	%edx, %ebx
	andl	$7, %ebx
	subq	%rbx, %r9
	movq	%r9, %r12
	addq	160(%rsp), %r12                 # 8-byte Folded Reload
	vxorps	%xmm10, %xmm10, %xmm10
	xorl	%ecx, %ecx
	movq	112(%rsp), %rdx                 # 8-byte Reload
	movq	72(%rsp), %r8                   # 8-byte Reload
	movq	104(%rsp), %r9                  # 8-byte Reload
	movq	96(%rsp), %r11                  # 8-byte Reload
	movq	88(%rsp), %rsi                  # 8-byte Reload
	.p2align	4
.LBB0_13:                               #   Parent Loop BB0_6 Depth=1
                                        # =>  This Inner Loop Header: Depth=2
	vmovaps	(%r8,%rcx,4), %ymm11
	vmulps	(%rdx,%rcx,4), %ymm11, %ymm12
	vmovaps	(%r9,%rcx,4), %ymm13
	vfmadd231ps	%ymm6, %ymm1, %ymm13    # ymm13 = (ymm1 * ymm6) + ymm13
	vmulps	%ymm13, %ymm11, %ymm13
	vmulps	(%r11,%rcx,4), %ymm11, %ymm11
	vmulps	%ymm13, %ymm13, %ymm14
	vfmadd231ps	%ymm12, %ymm12, %ymm14  # ymm14 = (ymm12 * ymm12) + ymm14
	vfmadd231ps	%ymm11, %ymm11, %ymm14  # ymm14 = (ymm11 * ymm11) + ymm14
	vmulps	%ymm7, %ymm14, %ymm14
	vmovaps	%ymm14, 256(%rsp,%rcx,4)
	vmovaps	%ymm12, (%rdx,%rcx,4)
	vmovaps	%ymm13, (%r9,%rcx,4)
	vmovaps	%ymm11, (%r11,%rcx,4)
	vfmadd213ps	(%rsi,%rcx,4), %ymm1, %ymm12 # ymm12 = (ymm1 * ymm12) + mem
	vfmadd213ps	(%r10,%rcx,4), %ymm1, %ymm13 # ymm13 = (ymm1 * ymm13) + mem
	vfmadd213ps	(%rdi,%rcx,4), %ymm1, %ymm11 # ymm11 = (ymm1 * ymm11) + mem
	vmovaps	%ymm12, (%rsi,%rcx,4)
	vmovaps	%ymm13, (%r10,%rcx,4)
	vmovaps	%ymm11, (%rdi,%rcx,4)
	vcmpleps	%ymm9, %ymm13, %ymm11
	vandps	%ymm8, %ymm11, %ymm11
	vorps	%ymm11, %ymm10, %ymm10
	addq	$8, %rcx
	cmpq	%rcx, %rax
	jne	.LBB0_13
# %bb.14:                               #   in Loop: Header=BB0_6 Depth=1
	vextractf128	$1, %ymm10, %xmm11
	vorps	%xmm11, %xmm10, %xmm10
	vshufps	$238, %xmm10, %xmm10, %xmm11    # xmm11 = xmm10[2,3,2,3]
	vorps	%xmm11, %xmm10, %xmm10
	vshufps	$85, %xmm10, %xmm10, %xmm11     # xmm11 = xmm10[1,1,1,1]
	vorps	%xmm11, %xmm10, %xmm10
	vmovd	%xmm10, %eax
	testq	%rbx, %rbx
	je	.LBB0_17
.LBB0_15:                               #   in Loop: Header=BB0_6 Depth=1
	movq	56(%rsp), %rdx                  # 8-byte Reload
	subq	%r12, %rdx
	movq	232(%rsp), %rcx                 # 8-byte Reload
	leaq	(%rcx,%r12,4), %rcx
	movq	240(%rsp), %r8                  # 8-byte Reload
	leaq	(%r8,%r12,4), %rbx
	movq	248(%rsp), %r8                  # 8-byte Reload
	leaq	(%r8,%r12,4), %r11
	movq	80(%rsp), %r8                   # 8-byte Reload
	leaq	(%r8,%r12,4), %rsi
	movq	208(%rsp), %r8                  # 8-byte Reload
	leaq	(%r8,%r12,4), %r8
	movq	216(%rsp), %r9                  # 8-byte Reload
	leaq	(%r9,%r12,4), %r9
	movq	200(%rsp), %r14                 # 8-byte Reload
	leaq	(%r14,%r12,4), %r15
	movq	224(%rsp), %r14                 # 8-byte Reload
	leaq	(%r14,%r12,4), %r12
	xorl	%r13d, %r13d
	.p2align	4
.LBB0_16:                               #   Parent Loop BB0_6 Depth=1
                                        # =>  This Inner Loop Header: Depth=2
	vmovss	(%r15,%r13,4), %xmm10           # xmm10 = mem[0],zero,zero,zero
	vmulss	(%r12,%r13,4), %xmm10, %xmm11
	vmovss	(%r9,%r13,4), %xmm12            # xmm12 = mem[0],zero,zero,zero
	vfmadd231ss	%xmm2, %xmm0, %xmm12    # xmm12 = (xmm0 * xmm2) + xmm12
	vmulss	%xmm12, %xmm10, %xmm12
	vmulss	(%r8,%r13,4), %xmm10, %xmm10
	vmulss	%xmm12, %xmm12, %xmm13
	vfmadd231ss	%xmm11, %xmm11, %xmm13  # xmm13 = (xmm11 * xmm11) + xmm13
	vfmadd231ss	%xmm10, %xmm10, %xmm13  # xmm13 = (xmm10 * xmm10) + xmm13
	vmulss	%xmm3, %xmm13, %xmm13
	vmovss	%xmm13, (%rsi,%r13,4)
	vmovss	%xmm11, (%r12,%r13,4)
	vmovss	%xmm12, (%r9,%r13,4)
	vmovss	%xmm10, (%r8,%r13,4)
	vfmadd213ss	(%r11,%r13,4), %xmm0, %xmm11 # xmm11 = (xmm0 * xmm11) + mem
	vfmadd213ss	(%rbx,%r13,4), %xmm0, %xmm12 # xmm12 = (xmm0 * xmm12) + mem
	vfmadd213ss	(%rcx,%r13,4), %xmm0, %xmm10 # xmm10 = (xmm0 * xmm10) + mem
	vmovss	%xmm11, (%r11,%r13,4)
	vmovss	%xmm12, (%rbx,%r13,4)
	vmovss	%xmm10, (%rcx,%r13,4)
	xorl	%r14d, %r14d
	vucomiss	%xmm12, %xmm4
	setae	%r14b
	orl	%r14d, %eax
	incq	%r13
	cmpq	%r13, %rdx
	jne	.LBB0_16
.LBB0_17:                               #   in Loop: Header=BB0_6 Depth=1
	testl	%eax, %eax
	jne	.LBB0_19
# %bb.18:                               #   in Loop: Header=BB0_6 Depth=1
	movq	144(%rsp), %rsi                 # 8-byte Reload
	movq	120(%rsp), %r11                 # 8-byte Reload
	movq	152(%rsp), %r12                 # 8-byte Reload
	movq	80(%rsp), %r9                   # 8-byte Reload
	movq	112(%rsp), %r13                 # 8-byte Reload
	movq	72(%rsp), %r8                   # 8-byte Reload
	movq	104(%rsp), %r14                 # 8-byte Reload
	movq	96(%rsp), %r15                  # 8-byte Reload
	movq	88(%rsp), %rbx                  # 8-byte Reload
	movq	64(%rsp), %rdx                  # 8-byte Reload
	movq	48(%rsp), %rcx                  # 8-byte Reload
	jmp	.LBB0_26
.LBB0_19:                               #   in Loop: Header=BB0_6 Depth=1
	movq	64(%rsp), %rdx                  # 8-byte Reload
	addq	%rdx, 56(%rsp)                  # 8-byte Folded Spill
	xorl	%eax, %eax
	movq	144(%rsp), %rsi                 # 8-byte Reload
	movq	120(%rsp), %r11                 # 8-byte Reload
	movq	152(%rsp), %r12                 # 8-byte Reload
	movq	80(%rsp), %r9                   # 8-byte Reload
	movq	112(%rsp), %r13                 # 8-byte Reload
	movq	72(%rsp), %r8                   # 8-byte Reload
	movq	104(%rsp), %r14                 # 8-byte Reload
	movq	96(%rsp), %r15                  # 8-byte Reload
	movq	88(%rsp), %rbx                  # 8-byte Reload
	movq	48(%rsp), %rcx                  # 8-byte Reload
.LBB0_20:                               #   Parent Loop BB0_6 Depth=1
                                        # =>  This Inner Loop Header: Depth=2
	vucomiss	(%r10,%rax,4), %xmm4
	jae	.LBB0_21
# %bb.25:                               #   in Loop: Header=BB0_20 Depth=2
	incq	%rax
	cmpq	%rax, 56(%rsp)                  # 8-byte Folded Reload
	jne	.LBB0_20
	jmp	.LBB0_26
.LBB0_21:                               #   in Loop: Header=BB0_20 Depth=2
	vmovss	(%r14,%rax,4), %xmm10           # xmm10 = mem[0],zero,zero,zero
	movl	$0, (%r10,%rax,4)
	vmulss	%xmm5, %xmm10, %xmm10
	vmovss	%xmm10, (%r14,%rax,4)
	movl	40(%rsp), %ecx                  # 4-byte Reload
	cmpq	%rcx, 176(%rsp)                 # 8-byte Folded Reload
	ja	.LBB0_23
# %bb.22:                               #   in Loop: Header=BB0_20 Depth=2
	movb	$1, %cl
	movq	%rcx, 136(%rsp)                 # 8-byte Spill
.LBB0_24:                               #   in Loop: Header=BB0_20 Depth=2
	movq	48(%rsp), %rcx                  # 8-byte Reload
	incq	%rax
	cmpq	%rax, 56(%rsp)                  # 8-byte Folded Reload
	jne	.LBB0_20
	jmp	.LBB0_26
.LBB0_23:                               #   in Loop: Header=BB0_20 Depth=2
	leaq	(%rcx,%rcx,4), %rcx
	movq	(%r11,%rax,8), %rdx
	vmovss	(%rbx,%rax,4), %xmm10           # xmm10 = mem[0],zero,zero,zero
	vmovss	(%rdi,%rax,4), %xmm11           # xmm11 = mem[0],zero,zero,zero
	vmovss	256(%rsp,%rax,4), %xmm12        # xmm12 = mem[0],zero,zero,zero
	movq	168(%rsp), %r8                  # 8-byte Reload
	movq	%rdx, (%r8,%rcx,4)
	movq	64(%rsp), %rdx                  # 8-byte Reload
	vmovss	%xmm10, 8(%r8,%rcx,4)
	vmovss	%xmm11, 12(%r8,%rcx,4)
	vmovss	%xmm12, 16(%r8,%rcx,4)
	movq	72(%rsp), %r8                   # 8-byte Reload
	movq	40(%rsp), %rcx                  # 8-byte Reload
	incl	%ecx
	movq	%rcx, 40(%rsp)                  # 8-byte Spill
	jmp	.LBB0_24
.LBB0_3:
	movl	$0, 8(%r11)
	movq	$0, (%r11)
	jmp	.LBB0_29
.LBB0_8:
	movq	40(%rsp), %rdx                  # 8-byte Reload
	movl	%edx, %eax
	leaq	(%rax,%rax,4), %rcx
	movq	184(%rsp), %r8                  # 8-byte Reload
	leaq	(%r8,%rcx,4), %rcx
	movq	192(%rsp), %r8                  # 8-byte Reload
	movq	%rcx, 16(%r8)
	addq	%rax, 24(%r8)
	movq	136(%rsp), %rax                 # 8-byte Reload
	testb	$1, %al
	notb	%al
	jne	.LBB0_9
.LBB0_10:
	movq	128(%rsp), %r11                 # 8-byte Reload
	andb	$1, %al
	jmp	.LBB0_28
.LBB0_27:
	movq	%rdi, 16(%r9)
	xorl	%edx, %edx
	movb	$1, %al
.LBB0_28:
	movb	$0, 3(%r11)
	movw	$0, 1(%r11)
	movl	$0, 8(%r11)
	movb	%al, (%r11)
	movl	%edx, 4(%r11)
.LBB0_29:
	movq	%r11, %rax
	vmovaps	1184(%rbp), %xmm6               # 16-byte Reload
	vmovaps	1200(%rbp), %xmm7               # 16-byte Reload
	vmovaps	1216(%rbp), %xmm8               # 16-byte Reload
	vmovaps	1232(%rbp), %xmm9               # 16-byte Reload
	vmovaps	1248(%rbp), %xmm10              # 16-byte Reload
	vmovaps	1264(%rbp), %xmm11              # 16-byte Reload
	vmovaps	1280(%rbp), %xmm12              # 16-byte Reload
	vmovaps	1296(%rbp), %xmm13              # 16-byte Reload
	vmovaps	1312(%rbp), %xmm14              # 16-byte Reload
	.seh_startepilogue
	leaq	1336(%rbp), %rsp
	popq	%rbx
	popq	%rdi
	popq	%rsi
	popq	%r12
	popq	%r13
	popq	%r14
	popq	%r15
	popq	%rbp
	.seh_endepilogue
	vzeroupper
	retq
.LBB0_1:
	movabsq	$4294967296, %rax               # imm = 0x100000000
	movq	%rax, (%r11)
	movl	$0, 8(%r11)
	jmp	.LBB0_29
.LBB0_9:
	movl	$2, %edx
	jmp	.LBB0_10
	.seh_endproc
                                        # -- End function
	.def	engine_ballistics_consume_ground_impacts;
	.scl	2;
	.type	32;
	.endef
	.globl	__real@3f666666                 # -- Begin function engine_ballistics_consume_ground_impacts
	.section	.rdata,"dr",discard,__real@3f666666
	.p2align	2, 0x0
__real@3f666666:
	.long	0x3f666666                      # float 0.899999976
	.text
	.globl	engine_ballistics_consume_ground_impacts
	.p2align	4
engine_ballistics_consume_ground_impacts: # @engine_ballistics_consume_ground_impacts
.seh_proc engine_ballistics_consume_ground_impacts
# %bb.0:
	pushq	%rsi
	.seh_pushreg %rsi
	.seh_endprologue
	movq	24(%rcx), %rax
	testq	%rax, %rax
	je	.LBB1_1
# %bb.3:
	movq	8(%rdx), %r8
	movq	16(%rdx), %r9
	movq	32(%rdx), %r10
	movq	(%rcx), %rcx
	movq	(%rdx), %rdx
	vxorps	%xmm0, %xmm0, %xmm0
	vmovss	__real@3f666666(%rip), %xmm1    # xmm1 = [8.99999976E-1,0.0E+0,0.0E+0,0.0E+0]
	.p2align	4
.LBB1_4:                                # =>This Inner Loop Header: Depth=1
	movl	(%rcx), %r11d
	cmpq	%r11, %rdx
	jbe	.LBB1_7
# %bb.5:                                #   in Loop: Header=BB1_4 Depth=1
	movl	4(%r8,%r11,8), %esi
	cmpl	4(%rcx), %esi
	jne	.LBB1_7
# %bb.6:                                #   in Loop: Header=BB1_4 Depth=1
	vmulss	(%r9,%r11,4), %xmm1, %xmm2
	vmovss	%xmm2, (%r9,%r11,4)
	vmulss	(%r10,%r11,4), %xmm1, %xmm2
	vmovss	%xmm2, (%r10,%r11,4)
.LBB1_7:                                #   in Loop: Header=BB1_4 Depth=1
	vaddss	16(%rcx), %xmm0, %xmm0
	addq	$20, %rcx
	decq	%rax
	jne	.LBB1_4
# %bb.2:
	.seh_startepilogue
	popq	%rsi
	.seh_endepilogue
	retq
.LBB1_1:
	vxorps	%xmm0, %xmm0, %xmm0
	.seh_startepilogue
	popq	%rsi
	.seh_endepilogue
	retq
	.seh_endproc
                                        # -- End function
	.def	main;
	.scl	2;
	.type	32;
	.endef
	.globl	__ymm@0000000700000006000000050000000400000003000000020000000100000000 # -- Begin function main
	.section	.rdata,"dr",discard,__ymm@0000000700000006000000050000000400000003000000020000000100000000
	.p2align	5, 0x0
__ymm@0000000700000006000000050000000400000003000000020000000100000000:
	.long	0                               # 0x0
	.long	1                               # 0x1
	.long	2                               # 0x2
	.long	3                               # 0x3
	.long	4                               # 0x4
	.long	5                               # 0x5
	.long	6                               # 0x6
	.long	7                               # 0x7
	.globl	__real@00000008
	.section	.rdata,"dr",discard,__real@00000008
	.p2align	2, 0x0
__real@00000008:
	.long	8                               # 0x8
	.globl	__real@00000010
	.section	.rdata,"dr",discard,__real@00000010
	.p2align	2, 0x0
__real@00000010:
	.long	16                              # 0x10
	.globl	__real@00000018
	.section	.rdata,"dr",discard,__real@00000018
	.p2align	2, 0x0
__real@00000018:
	.long	24                              # 0x18
	.globl	__real@3c23d70a
	.section	.rdata,"dr",discard,__real@3c23d70a
	.p2align	2, 0x0
__real@3c23d70a:
	.long	0x3c23d70a                      # float 0.00999999977
	.globl	__real@41200000
	.section	.rdata,"dr",discard,__real@41200000
	.p2align	2, 0x0
__real@41200000:
	.long	0x41200000                      # float 10
	.globl	__real@c1a00000
	.section	.rdata,"dr",discard,__real@c1a00000
	.p2align	2, 0x0
__real@c1a00000:
	.long	0xc1a00000                      # float -20
	.globl	__real@3f800000
	.section	.rdata,"dr",discard,__real@3f800000
	.p2align	2, 0x0
__real@3f800000:
	.long	0x3f800000                      # float 1
	.globl	__real@00000020
	.section	.rdata,"dr",discard,__real@00000020
	.p2align	2, 0x0
__real@00000020:
	.long	32                              # 0x20
	.globl	__ymm@0000000100002707000000010000270600000001000027050000000100002704
	.section	.rdata,"dr",discard,__ymm@0000000100002707000000010000270600000001000027050000000100002704
	.p2align	5, 0x0
__ymm@0000000100002707000000010000270600000001000027050000000100002704:
	.long	9988                            # 0x2704
	.long	1                               # 0x1
	.long	9989                            # 0x2705
	.long	1                               # 0x1
	.long	9990                            # 0x2706
	.long	1                               # 0x1
	.long	9991                            # 0x2707
	.long	1                               # 0x1
	.globl	__ymm@0000000100002703000000010000270200000001000027010000000100002700
	.section	.rdata,"dr",discard,__ymm@0000000100002703000000010000270200000001000027010000000100002700
	.p2align	5, 0x0
__ymm@0000000100002703000000010000270200000001000027010000000100002700:
	.long	9984                            # 0x2700
	.long	1                               # 0x1
	.long	9985                            # 0x2701
	.long	1                               # 0x1
	.long	9986                            # 0x2702
	.long	1                               # 0x1
	.long	9987                            # 0x2703
	.long	1                               # 0x1
	.globl	__ymm@000000010000270f000000010000270e000000010000270d000000010000270c
	.section	.rdata,"dr",discard,__ymm@000000010000270f000000010000270e000000010000270d000000010000270c
	.p2align	5, 0x0
__ymm@000000010000270f000000010000270e000000010000270d000000010000270c:
	.long	9996                            # 0x270c
	.long	1                               # 0x1
	.long	9997                            # 0x270d
	.long	1                               # 0x1
	.long	9998                            # 0x270e
	.long	1                               # 0x1
	.long	9999                            # 0x270f
	.long	1                               # 0x1
	.globl	__ymm@000000010000270b000000010000270a00000001000027090000000100002708
	.section	.rdata,"dr",discard,__ymm@000000010000270b000000010000270a00000001000027090000000100002708
	.p2align	5, 0x0
__ymm@000000010000270b000000010000270a00000001000027090000000100002708:
	.long	9992                            # 0x2708
	.long	1                               # 0x1
	.long	9993                            # 0x2709
	.long	1                               # 0x1
	.long	9994                            # 0x270a
	.long	1                               # 0x1
	.long	9995                            # 0x270b
	.long	1                               # 0x1
	.globl	__xmm@00070006000500040003000200010000
	.section	.rdata,"dr",discard,__xmm@00070006000500040003000200010000
	.p2align	4, 0x0
__xmm@00070006000500040003000200010000:
	.short	0                               # 0x0
	.short	1                               # 0x1
	.short	2                               # 0x2
	.short	3                               # 0x3
	.short	4                               # 0x4
	.short	5                               # 0x5
	.short	6                               # 0x6
	.short	7                               # 0x7
	.globl	__xmm@83138313831383138313831383138313
	.section	.rdata,"dr",discard,__xmm@83138313831383138313831383138313
	.p2align	4, 0x0
__xmm@83138313831383138313831383138313:
	.short	33555                           # 0x8313
	.short	33555                           # 0x8313
	.short	33555                           # 0x8313
	.short	33555                           # 0x8313
	.short	33555                           # 0x8313
	.short	33555                           # 0x8313
	.short	33555                           # 0x8313
	.short	33555                           # 0x8313
	.globl	__xmm@00fa00fa00fa00fa00fa00fa00fa00fa
	.section	.rdata,"dr",discard,__xmm@00fa00fa00fa00fa00fa00fa00fa00fa
	.p2align	4, 0x0
__xmm@00fa00fa00fa00fa00fa00fa00fa00fa:
	.short	250                             # 0xfa
	.short	250                             # 0xfa
	.short	250                             # 0xfa
	.short	250                             # 0xfa
	.short	250                             # 0xfa
	.short	250                             # 0xfa
	.short	250                             # 0xfa
	.short	250                             # 0xfa
	.globl	__real@40a00000
	.section	.rdata,"dr",discard,__real@40a00000
	.p2align	2, 0x0
__real@40a00000:
	.long	0x40a00000                      # float 5
	.globl	__real@3f000000
	.section	.rdata,"dr",discard,__real@3f000000
	.p2align	2, 0x0
__real@3f000000:
	.long	0x3f000000                      # float 0.5
	.globl	__real@42200000
	.section	.rdata,"dr",discard,__real@42200000
	.p2align	2, 0x0
__real@42200000:
	.long	0x42200000                      # float 40
	.globl	__xmm@88898889888988898889888988898889
	.section	.rdata,"dr",discard,__xmm@88898889888988898889888988898889
	.p2align	4, 0x0
__xmm@88898889888988898889888988898889:
	.short	34953                           # 0x8889
	.short	34953                           # 0x8889
	.short	34953                           # 0x8889
	.short	34953                           # 0x8889
	.short	34953                           # 0x8889
	.short	34953                           # 0x8889
	.short	34953                           # 0x8889
	.short	34953                           # 0x8889
	.globl	__xmm@001e001e001e001e001e001e001e001e
	.section	.rdata,"dr",discard,__xmm@001e001e001e001e001e001e001e001e
	.p2align	4, 0x0
__xmm@001e001e001e001e001e001e001e001e:
	.short	30                              # 0x1e
	.short	30                              # 0x1e
	.short	30                              # 0x1e
	.short	30                              # 0x1e
	.short	30                              # 0x1e
	.short	30                              # 0x1e
	.short	30                              # 0x1e
	.short	30                              # 0x1e
	.globl	__xmm@000f000f000f000f000f000f000f000f
	.section	.rdata,"dr",discard,__xmm@000f000f000f000f000f000f000f000f
	.p2align	4, 0x0
__xmm@000f000f000f000f000f000f000f000f:
	.short	15                              # 0xf
	.short	15                              # 0xf
	.short	15                              # 0xf
	.short	15                              # 0xf
	.short	15                              # 0xf
	.short	15                              # 0xf
	.short	15                              # 0xf
	.short	15                              # 0xf
	.globl	__real@41a00000
	.section	.rdata,"dr",discard,__real@41a00000
	.p2align	2, 0x0
__real@41a00000:
	.long	0x41a00000                      # float 20
	.globl	__real@3f7fbe77
	.section	.rdata,"dr",discard,__real@3f7fbe77
	.p2align	2, 0x0
__real@3f7fbe77:
	.long	0x3f7fbe77                      # float 0.999000012
	.globl	__xmm@00080008000800080008000800080008
	.section	.rdata,"dr",discard,__xmm@00080008000800080008000800080008
	.p2align	4, 0x0
__xmm@00080008000800080008000800080008:
	.short	8                               # 0x8
	.short	8                               # 0x8
	.short	8                               # 0x8
	.short	8                               # 0x8
	.short	8                               # 0x8
	.short	8                               # 0x8
	.short	8                               # 0x8
	.short	8                               # 0x8
	.globl	__real@412e848000000000
	.section	.rdata,"dr",discard,__real@412e848000000000
	.p2align	3, 0x0
__real@412e848000000000:
	.quad	0x412e848000000000              # double 1.0E+6
	.globl	__real@408f400000000000
	.section	.rdata,"dr",discard,__real@408f400000000000
	.p2align	3, 0x0
__real@408f400000000000:
	.quad	0x408f400000000000              # double 1000
	.globl	__real@40c3880000000000
	.section	.rdata,"dr",discard,__real@40c3880000000000
	.p2align	3, 0x0
__real@40c3880000000000:
	.quad	0x40c3880000000000              # double 1.0E+4
	.globl	__real@42480000
	.section	.rdata,"dr",discard,__real@42480000
	.p2align	2, 0x0
__real@42480000:
	.long	0x42480000                      # float 50
	.globl	__xmm@00000000000000004530000043300000
	.section	.rdata,"dr",discard,__xmm@00000000000000004530000043300000
	.p2align	4, 0x0
__xmm@00000000000000004530000043300000:
	.long	1127219200                      # 0x43300000
	.long	1160773632                      # 0x45300000
	.long	0                               # 0x0
	.long	0                               # 0x0
	.globl	__xmm@45300000000000004330000000000000
	.section	.rdata,"dr",discard,__xmm@45300000000000004330000000000000
	.p2align	4, 0x0
__xmm@45300000000000004330000000000000:
	.quad	0x4330000000000000              # double 4503599627370496
	.quad	0x4530000000000000              # double 1.9342813113834067E+25
	.globl	__real@4059000000000000
	.section	.rdata,"dr",discard,__real@4059000000000000
	.p2align	3, 0x0
__real@4059000000000000:
	.quad	0x4059000000000000              # double 100
	.globl	__real@416312d000000000
	.section	.rdata,"dr",discard,__real@416312d000000000
	.p2align	3, 0x0
__real@416312d000000000:
	.quad	0x416312d000000000              # double 1.0E+7
	.text
	.globl	main
	.p2align	4
main:                                   # @main
.seh_proc main
# %bb.0:
	pushq	%rbp
	.seh_pushreg %rbp
	pushq	%r15
	.seh_pushreg %r15
	pushq	%r14
	.seh_pushreg %r14
	pushq	%r13
	.seh_pushreg %r13
	pushq	%r12
	.seh_pushreg %r12
	pushq	%rsi
	.seh_pushreg %rsi
	pushq	%rdi
	.seh_pushreg %rdi
	pushq	%rbx
	.seh_pushreg %rbx
	subq	$504, %rsp                      # imm = 0x1F8
	.seh_stackalloc 504
	leaq	128(%rsp), %rbp
	.seh_setframe %rbp, 128
	vmovaps	%xmm15, 352(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm15, 480
	vmovdqa	%xmm14, 336(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm14, 464
	vmovaps	%xmm13, 320(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm13, 448
	vmovdqa	%xmm12, 304(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm12, 432
	vmovdqa	%xmm11, 288(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm11, 416
	vmovdqa	%xmm10, 272(%rbp)               # 16-byte Spill
	.seh_savexmm %xmm10, 400
	vmovdqa	%xmm9, 256(%rbp)                # 16-byte Spill
	.seh_savexmm %xmm9, 384
	vmovdqa	%xmm8, 240(%rbp)                # 16-byte Spill
	.seh_savexmm %xmm8, 368
	vmovapd	%xmm7, 224(%rbp)                # 16-byte Spill
	.seh_savexmm %xmm7, 352
	vmovapd	%xmm6, 208(%rbp)                # 16-byte Spill
	.seh_savexmm %xmm6, 336
	.seh_endprologue
	andq	$-64, %rsp
	movl	$1015580826, 52(%rsp)           # imm = 0x3C88889A
	leaq	"??_C@_0IM@ECCBDHPO@?$FLCHECK?50?5PASS?$FN?5Compact?5SoA?5Descr@"(%rip), %rcx
	movl	$64, %edx
	movl	$64, %r8d
	callq	printf
	vmovaps	.L__const.main.frame_arena+32(%rip), %ymm0
	vmovaps	%ymm0, 160(%rsp)
	vmovapd	.L__const.main.frame_arena(%rip), %ymm0
	vmovapd	%ymm0, 128(%rsp)
	leaq	52(%rsp), %rax
	movq	%rax, 32(%rsp)
	leaq	.L__const.main.pos_col(%rip), %rdx
	leaq	.L__const.main.bad_vel(%rip), %r8
	leaq	116(%rsp), %rcx
	leaq	128(%rsp), %r9
	vzeroupper
	callq	engine_ballistics_step_ballistics_10k
	movl	$1, %eax
	cmpb	$0, 116(%rsp)
	jne	.LBB2_40
# %bb.1:
	cmpl	$1, 120(%rsp)
	jne	.LBB2_40
# %bb.2:
	leaq	.Lstr(%rip), %rcx
	callq	puts
	leaq	g_px(%rip), %rcx
	xorl	%ebx, %ebx
	movl	$40000, %r8d                    # imm = 0x9C40
	xorl	%edx, %edx
	callq	memset
	leaq	g_pz(%rip), %rcx
	movl	$40000, %r8d                    # imm = 0x9C40
	xorl	%edx, %edx
	callq	memset
	vmovdqa	__ymm@0000000700000006000000050000000400000003000000020000000100000000(%rip), %ymm4 # ymm4 = [0,1,2,3,4,5,6,7]
	vpbroadcastd	__real@00000008(%rip), %ymm13 # ymm13 = [8,8,8,8,8,8,8,8]
	vpbroadcastd	__real@00000010(%rip), %ymm5 # ymm5 = [16,16,16,16,16,16,16,16]
	vpbroadcastd	__real@00000018(%rip), %ymm6 # ymm6 = [24,24,24,24,24,24,24,24]
	vpbroadcastd	__real@00000001(%rip), %ymm14 # ymm14 = [1,1,1,1,1,1,1,1]
	leaq	g_handles(%rip), %rsi
	leaq	g_py(%rip), %r14
	vbroadcastss	__real@3c23d70a(%rip), %ymm1 # ymm1 = [9.99999977E-3,9.99999977E-3,9.99999977E-3,9.99999977E-3,9.99999977E-3,9.99999977E-3,9.99999977E-3,9.99999977E-3]
	leaq	g_vx(%rip), %r13
	vbroadcastss	__real@41200000(%rip), %ymm0 # ymm0 = [1.0E+1,1.0E+1,1.0E+1,1.0E+1,1.0E+1,1.0E+1,1.0E+1,1.0E+1]
	leaq	g_vy(%rip), %r15
	vbroadcastss	__real@c1a00000(%rip), %ymm2 # ymm2 = [-2.0E+1,-2.0E+1,-2.0E+1,-2.0E+1,-2.0E+1,-2.0E+1,-2.0E+1,-2.0E+1]
	leaq	g_vz(%rip), %rdi
	leaq	g_drag(%rip), %r12
	vpbroadcastd	__real@3f800000(%rip), %ymm3 # ymm3 = [1.0E+0,1.0E+0,1.0E+0,1.0E+0,1.0E+0,1.0E+0,1.0E+0,1.0E+0]
	vpbroadcastd	__real@00000020(%rip), %ymm7 # ymm7 = [32,32,32,32,32,32,32,32]
	.p2align	4
.LBB2_3:                                # =>This Inner Loop Header: Depth=1
	vpaddd	%ymm4, %ymm13, %ymm8
	vpaddd	%ymm5, %ymm4, %ymm9
	vpaddd	%ymm6, %ymm4, %ymm10
	vpmovzxdq	%xmm4, %ymm11           # ymm11 = xmm4[0],zero,xmm4[1],zero,xmm4[2],zero,xmm4[3],zero
	vpblendd	$170, %ymm14, %ymm11, %ymm11    # ymm11 = ymm11[0],ymm14[1],ymm11[2],ymm14[3],ymm11[4],ymm14[5],ymm11[6],ymm14[7]
	vextracti128	$1, %ymm4, %xmm12
	vpmovzxdq	%xmm12, %ymm12          # ymm12 = xmm12[0],zero,xmm12[1],zero,xmm12[2],zero,xmm12[3],zero
	vpblendd	$170, %ymm14, %ymm12, %ymm12    # ymm12 = ymm12[0],ymm14[1],ymm12[2],ymm14[3],ymm12[4],ymm14[5],ymm12[6],ymm14[7]
	vmovdqa	%ymm12, 32(%rsi,%rbx,2)
	vmovdqa	%ymm11, (%rsi,%rbx,2)
	vpmovzxdq	%xmm8, %ymm11           # ymm11 = xmm8[0],zero,xmm8[1],zero,xmm8[2],zero,xmm8[3],zero
	vpblendd	$170, %ymm14, %ymm11, %ymm11    # ymm11 = ymm11[0],ymm14[1],ymm11[2],ymm14[3],ymm11[4],ymm14[5],ymm11[6],ymm14[7]
	vextracti128	$1, %ymm8, %xmm8
	vpmovzxdq	%xmm8, %ymm8            # ymm8 = xmm8[0],zero,xmm8[1],zero,xmm8[2],zero,xmm8[3],zero
	vpblendd	$170, %ymm14, %ymm8, %ymm8      # ymm8 = ymm8[0],ymm14[1],ymm8[2],ymm14[3],ymm8[4],ymm14[5],ymm8[6],ymm14[7]
	vmovdqa	%ymm8, 96(%rsi,%rbx,2)
	vmovdqa	%ymm11, 64(%rsi,%rbx,2)
	vpmovzxdq	%xmm9, %ymm8            # ymm8 = xmm9[0],zero,xmm9[1],zero,xmm9[2],zero,xmm9[3],zero
	vpblendd	$170, %ymm14, %ymm8, %ymm8      # ymm8 = ymm8[0],ymm14[1],ymm8[2],ymm14[3],ymm8[4],ymm14[5],ymm8[6],ymm14[7]
	vextracti128	$1, %ymm9, %xmm9
	vpmovzxdq	%xmm9, %ymm9            # ymm9 = xmm9[0],zero,xmm9[1],zero,xmm9[2],zero,xmm9[3],zero
	vpblendd	$170, %ymm14, %ymm9, %ymm9      # ymm9 = ymm9[0],ymm14[1],ymm9[2],ymm14[3],ymm9[4],ymm14[5],ymm9[6],ymm14[7]
	vmovdqa	%ymm9, 160(%rsi,%rbx,2)
	vmovdqa	%ymm8, 128(%rsi,%rbx,2)
	vpmovzxdq	%xmm10, %ymm8           # ymm8 = xmm10[0],zero,xmm10[1],zero,xmm10[2],zero,xmm10[3],zero
	vpblendd	$170, %ymm14, %ymm8, %ymm8      # ymm8 = ymm8[0],ymm14[1],ymm8[2],ymm14[3],ymm8[4],ymm14[5],ymm8[6],ymm14[7]
	vextracti128	$1, %ymm10, %xmm9
	vpmovzxdq	%xmm9, %ymm9            # ymm9 = xmm9[0],zero,xmm9[1],zero,xmm9[2],zero,xmm9[3],zero
	vpblendd	$170, %ymm14, %ymm9, %ymm9      # ymm9 = ymm9[0],ymm14[1],ymm9[2],ymm14[3],ymm9[4],ymm14[5],ymm9[6],ymm14[7]
	vmovdqa	%ymm9, 224(%rsi,%rbx,2)
	vmovdqa	%ymm8, 192(%rsi,%rbx,2)
	vmovapd	%ymm1, (%rbx,%r14)
	vmovapd	%ymm1, 32(%rbx,%r14)
	vmovapd	%ymm1, 64(%rbx,%r14)
	vmovapd	%ymm1, 96(%rbx,%r14)
	vmovaps	%ymm0, (%rbx,%r13)
	vmovaps	%ymm0, 32(%rbx,%r13)
	vmovaps	%ymm0, 64(%rbx,%r13)
	vmovaps	%ymm0, 96(%rbx,%r13)
	vmovaps	%ymm2, (%rbx,%r15)
	vmovaps	%ymm2, 32(%rbx,%r15)
	vmovaps	%ymm2, 64(%rbx,%r15)
	vmovaps	%ymm2, 96(%rbx,%r15)
	vmovaps	%ymm0, (%rbx,%rdi)
	vmovaps	%ymm0, 32(%rbx,%rdi)
	vmovaps	%ymm0, 64(%rbx,%rdi)
	vmovaps	%ymm0, 96(%rbx,%rdi)
	vmovdqa	%ymm3, (%rbx,%r12)
	vmovdqa	%ymm3, 32(%rbx,%r12)
	vmovdqa	%ymm3, 64(%rbx,%r12)
	vmovdqa	%ymm3, 96(%rbx,%r12)
	vpaddd	%ymm7, %ymm4, %ymm4
	subq	$-128, %rbx
	cmpq	$39936, %rbx                    # imm = 0x9C00
	jne	.LBB2_3
# %bb.4:
	vmovdqa	%ymm14, 192(%rsp)               # 32-byte Spill
	vmovdqa	%ymm13, 224(%rsp)               # 32-byte Spill
	vmovaps	__ymm@0000000100002707000000010000270600000001000027050000000100002704(%rip), %ymm4 # ymm4 = [9988,1,9989,1,9990,1,9991,1]
	vmovaps	%ymm4, g_handles+79904(%rip)
	vmovaps	__ymm@0000000100002703000000010000270200000001000027010000000100002700(%rip), %ymm4 # ymm4 = [9984,1,9985,1,9986,1,9987,1]
	vmovaps	%ymm4, g_handles+79872(%rip)
	vmovapd	%ymm1, g_py+39936(%rip)
	vmovaps	%ymm0, g_vx+39936(%rip)
	vmovaps	%ymm2, g_vy+39936(%rip)
	vmovaps	%ymm0, g_vz+39936(%rip)
	vmovdqa	%ymm3, g_drag+39936(%rip)
	vmovaps	__ymm@000000010000270f000000010000270e000000010000270d000000010000270c(%rip), %ymm4 # ymm4 = [9996,1,9997,1,9998,1,9999,1]
	vmovaps	%ymm4, g_handles+79968(%rip)
	vmovdqa	__ymm@000000010000270b000000010000270a00000001000027090000000100002708(%rip), %ymm4 # ymm4 = [9992,1,9993,1,9994,1,9995,1]
	vmovdqa	%ymm4, g_handles+79936(%rip)
	vmovapd	%ymm1, g_py+39968(%rip)
	vmovaps	%ymm0, g_vx+39968(%rip)
	vmovaps	%ymm2, g_vy+39968(%rip)
	vmovaps	%ymm0, g_vz+39968(%rip)
	vmovdqa	%ymm3, g_drag+39968(%rip)
	vmovaps	.L__const.main.tiny_arena+32(%rip), %ymm0
	vmovaps	%ymm0, 288(%rsp)
	vmovaps	.L__const.main.tiny_arena(%rip), %ymm0
	vmovaps	%ymm0, 256(%rsp)
	leaq	52(%rsp), %rax
	movq	%rax, 32(%rsp)
	leaq	.L__const.main.pos_col(%rip), %rdx
	leaq	.L__const.main.vel_col(%rip), %r8
	leaq	104(%rsp), %rcx
	leaq	256(%rsp), %r9
	vzeroupper
	callq	engine_ballistics_step_ballistics_10k
	cmpl	$2, 108(%rsp)
	setne	%cl
	orb	104(%rsp), %cl
	movl	$1, %edx
	vxorps	%xmm0, %xmm0, %xmm0
	.p2align	4
.LBB2_5:                                # =>This Inner Loop Header: Depth=1
	movl	$1, %eax
	vucomiss	-4(%r14,%rdx,4), %xmm0
	ja	.LBB2_40
# %bb.6:                                #   in Loop: Header=BB2_5 Depth=1
	vucomiss	-4(%r15,%rdx,4), %xmm0
	jae	.LBB2_40
# %bb.7:                                #   in Loop: Header=BB2_5 Depth=1
	vucomiss	(%r14,%rdx,4), %xmm0
	ja	.LBB2_40
# %bb.8:                                #   in Loop: Header=BB2_5 Depth=1
	vucomiss	(%r15,%rdx,4), %xmm0
	jae	.LBB2_40
# %bb.9:                                #   in Loop: Header=BB2_5 Depth=1
	addq	$2, %rdx
	cmpq	$10001, %rdx                    # imm = 0x2711
	jne	.LBB2_5
# %bb.10:
	testb	%cl, %cl
	jne	.LBB2_40
# %bb.11:
	leaq	.Lstr.1(%rip), %rcx
	callq	puts
	callq	*__imp_GetCurrentProcess(%rip)
	movq	%rax, %rcx
	movl	$128, %edx
	callq	*__imp_SetPriorityClass(%rip)
	callq	*__imp_GetCurrentThread(%rip)
	movq	%rax, %rcx
	movl	$2, %edx
	callq	*__imp_SetThreadPriority(%rip)
	leaq	g_px(%rip), %rcx
	xorl	%ebx, %ebx
	movl	$40000, %r8d                    # imm = 0x9C40
	xorl	%edx, %edx
	callq	memset
	leaq	g_pz(%rip), %rcx
	movl	$40000, %r8d                    # imm = 0x9C40
	xorl	%edx, %edx
	callq	memset
	vmovdqa	__xmm@00070006000500040003000200010000(%rip), %xmm0 # xmm0 = [0,1,2,3,4,5,6,7]
	vmovdqa	__ymm@0000000700000006000000050000000400000003000000020000000100000000(%rip), %ymm1 # ymm1 = [0,1,2,3,4,5,6,7]
	vmovdqa	__xmm@83138313831383138313831383138313(%rip), %xmm6 # xmm6 = [33555,33555,33555,33555,33555,33555,33555,33555]
	vmovdqa	__xmm@00fa00fa00fa00fa00fa00fa00fa00fa(%rip), %xmm7 # xmm7 = [250,250,250,250,250,250,250,250]
	vbroadcastss	__real@40a00000(%rip), %ymm5 # ymm5 = [5.0E+0,5.0E+0,5.0E+0,5.0E+0,5.0E+0,5.0E+0,5.0E+0,5.0E+0]
	vbroadcastss	__real@3f000000(%rip), %ymm12 # ymm12 = [5.0E-1,5.0E-1,5.0E-1,5.0E-1,5.0E-1,5.0E-1,5.0E-1,5.0E-1]
	vbroadcastss	__real@42200000(%rip), %ymm13 # ymm13 = [4.0E+1,4.0E+1,4.0E+1,4.0E+1,4.0E+1,4.0E+1,4.0E+1,4.0E+1]
	vmovdqa	__xmm@88898889888988898889888988898889(%rip), %xmm8 # xmm8 = [34953,34953,34953,34953,34953,34953,34953,34953]
	vmovdqa	__xmm@001e001e001e001e001e001e001e001e(%rip), %xmm9 # xmm9 = [30,30,30,30,30,30,30,30]
	vpbroadcastd	__real@41a00000(%rip), %ymm14 # ymm14 = [2.0E+1,2.0E+1,2.0E+1,2.0E+1,2.0E+1,2.0E+1,2.0E+1,2.0E+1]
	vmovdqa	__xmm@000f000f000f000f000f000f000f000f(%rip), %xmm10 # xmm10 = [15,15,15,15,15,15,15,15]
	vbroadcastss	__real@3f7fbe77(%rip), %ymm15 # ymm15 = [9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1]
	vmovdqa	__xmm@00080008000800080008000800080008(%rip), %xmm11 # xmm11 = [8,8,8,8,8,8,8,8]
	vmovdqa	224(%rsp), %ymm3                # 32-byte Reload
	vmovdqa	192(%rsp), %ymm4                # 32-byte Reload
	.p2align	4
.LBB2_12:                               # =>This Inner Loop Header: Depth=1
	vextracti128	$1, %ymm1, %xmm2
	vpmovzxdq	%xmm2, %ymm2            # ymm2 = xmm2[0],zero,xmm2[1],zero,xmm2[2],zero,xmm2[3],zero
	vpblendd	$170, %ymm4, %ymm2, %ymm2       # ymm2 = ymm2[0],ymm4[1],ymm2[2],ymm4[3],ymm2[4],ymm4[5],ymm2[6],ymm4[7]
	vmovdqa	%ymm2, 32(%rsi)
	vpmovzxdq	%xmm1, %ymm2            # ymm2 = xmm1[0],zero,xmm1[1],zero,xmm1[2],zero,xmm1[3],zero
	vpblendd	$170, %ymm4, %ymm2, %ymm2       # ymm2 = ymm2[0],ymm4[1],ymm2[2],ymm4[3],ymm2[4],ymm4[5],ymm2[6],ymm4[7]
	vmovdqa	%ymm2, (%rsi)
	vpsrlw	$1, %xmm0, %xmm2
	vpmulhuw	%xmm6, %xmm2, %xmm2
	vpsrlw	$6, %xmm2, %xmm2
	vpmullw	%xmm7, %xmm2, %xmm2
	vpsubw	%xmm2, %xmm0, %xmm2
	vpmovzxwd	%xmm2, %ymm2            # ymm2 = xmm2[0],zero,xmm2[1],zero,xmm2[2],zero,xmm2[3],zero,xmm2[4],zero,xmm2[5],zero,xmm2[6],zero,xmm2[7],zero
	vcvtdq2ps	%ymm2, %ymm2
	vfmadd213ps	%ymm5, %ymm12, %ymm2    # ymm2 = (ymm12 * ymm2) + ymm5
	vmovaps	%ymm2, (%rbx,%r14)
	vpmulhuw	%xmm0, %xmm8, %xmm2
	vpsrlw	$4, %xmm2, %xmm2
	vpmullw	%xmm2, %xmm9, %xmm2
	vpsubw	%xmm2, %xmm0, %xmm2
	vpaddw	%xmm2, %xmm10, %xmm2
	vpmovzxwd	%xmm2, %ymm2            # ymm2 = xmm2[0],zero,xmm2[1],zero,xmm2[2],zero,xmm2[3],zero,xmm2[4],zero,xmm2[5],zero,xmm2[6],zero,xmm2[7],zero
	vcvtdq2ps	%ymm2, %ymm2
	vmovaps	%ymm2, (%rbx,%r15)
	vmovaps	%ymm13, (%rbx,%r13)
	vmovdqa	%ymm14, (%rbx,%rdi)
	vmovaps	%ymm15, (%rbx,%r12)
	vpaddd	%ymm3, %ymm1, %ymm1
	vpaddw	%xmm0, %xmm11, %xmm0
	addq	$32, %rbx
	addq	$64, %rsi
	cmpq	$40000, %rbx                    # imm = 0x9C40
	jne	.LBB2_12
# %bb.13:
	vxorps	%xmm13, %xmm13, %xmm13
	vmovaps	%xmm13, 144(%rsp)
	leaq	56(%rsp), %rcx
	vzeroupper
	callq	*__imp_QueryPerformanceFrequency(%rip)
	leaq	72(%rsp), %rcx
	callq	*__imp_QueryPerformanceCounter(%rip)
	vcvtsi2sdq	72(%rsp), %xmm6, %xmm0
	vmulsd	__real@412e848000000000(%rip), %xmm0, %xmm0
	vcvtsi2sdq	56(%rsp), %xmm6, %xmm1
	vdivsd	%xmm1, %xmm0, %xmm12
	xorl	%esi, %esi
	xorl	%ebx, %ebx
	.p2align	4
.LBB2_14:                               # =>This Inner Loop Header: Depth=1
	leaq	52(%rsp), %rax
	movq	%rax, 32(%rsp)
	leaq	56(%rsp), %rcx
	leaq	.L__const.main.pos_col(%rip), %rdx
	leaq	.L__const.main.vel_col(%rip), %r8
	leaq	128(%rsp), %r9
	callq	engine_ballistics_step_ballistics_10k
	movl	60(%rsp), %edx
	cmpb	$1, 56(%rsp)
	jne	.LBB2_29
# %bb.15:                               #   in Loop: Header=BB2_14 Depth=1
	addq	%rdx, %rsi
	leaq	144(%rsp), %rax
	vmovaps	%xmm13, (%rax)
	incl	%ebx
	cmpl	$1000, %ebx                     # imm = 0x3E8
	jne	.LBB2_14
# %bb.16:
	vmovsd	%xmm12, 96(%rsp)                # 8-byte Spill
	leaq	56(%rsp), %rcx
	callq	*__imp_QueryPerformanceFrequency(%rip)
	leaq	72(%rsp), %rcx
	callq	*__imp_QueryPerformanceCounter(%rip)
	movq	72(%rsp), %rax
	movq	%rax, 80(%rsp)                  # 8-byte Spill
	movq	56(%rsp), %rax
	movq	%rax, 88(%rsp)                  # 8-byte Spill
	leaq	g_px(%rip), %rcx
	xorl	%ebx, %ebx
	movl	$40000, %r8d                    # imm = 0x9C40
	xorl	%edx, %edx
	callq	memset
	leaq	g_pz(%rip), %rcx
	movl	$40000, %r8d                    # imm = 0x9C40
	xorl	%edx, %edx
	callq	memset
	vmovdqa	__xmm@00070006000500040003000200010000(%rip), %xmm0 # xmm0 = [0,1,2,3,4,5,6,7]
	vmovdqa	__ymm@0000000700000006000000050000000400000003000000020000000100000000(%rip), %ymm1 # ymm1 = [0,1,2,3,4,5,6,7]
	leaq	g_handles(%rip), %rax
	vmovdqa	224(%rsp), %ymm3                # 32-byte Reload
	vmovdqa	192(%rsp), %ymm4                # 32-byte Reload
	vbroadcastss	__real@40a00000(%rip), %ymm5 # ymm5 = [5.0E+0,5.0E+0,5.0E+0,5.0E+0,5.0E+0,5.0E+0,5.0E+0,5.0E+0]
	vbroadcastss	__real@3f000000(%rip), %ymm13 # ymm13 = [5.0E-1,5.0E-1,5.0E-1,5.0E-1,5.0E-1,5.0E-1,5.0E-1,5.0E-1]
	vpbroadcastd	__real@42200000(%rip), %ymm14 # ymm14 = [4.0E+1,4.0E+1,4.0E+1,4.0E+1,4.0E+1,4.0E+1,4.0E+1,4.0E+1]
	vbroadcastss	__real@41a00000(%rip), %ymm15 # ymm15 = [2.0E+1,2.0E+1,2.0E+1,2.0E+1,2.0E+1,2.0E+1,2.0E+1,2.0E+1]
	vbroadcastss	__real@3f7fbe77(%rip), %ymm12 # ymm12 = [9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1,9.99000012E-1]
	.p2align	4
.LBB2_17:                               # =>This Inner Loop Header: Depth=1
	vextracti128	$1, %ymm1, %xmm2
	vpmovzxdq	%xmm2, %ymm2            # ymm2 = xmm2[0],zero,xmm2[1],zero,xmm2[2],zero,xmm2[3],zero
	vpblendd	$170, %ymm4, %ymm2, %ymm2       # ymm2 = ymm2[0],ymm4[1],ymm2[2],ymm4[3],ymm2[4],ymm4[5],ymm2[6],ymm4[7]
	vmovdqa	%ymm2, 32(%rax)
	vpmovzxdq	%xmm1, %ymm2            # ymm2 = xmm1[0],zero,xmm1[1],zero,xmm1[2],zero,xmm1[3],zero
	vpblendd	$170, %ymm4, %ymm2, %ymm2       # ymm2 = ymm2[0],ymm4[1],ymm2[2],ymm4[3],ymm2[4],ymm4[5],ymm2[6],ymm4[7]
	vmovdqa	%ymm2, (%rax)
	vpsrlw	$1, %xmm0, %xmm2
	vpmulhuw	%xmm6, %xmm2, %xmm2
	vpsrlw	$6, %xmm2, %xmm2
	vpmullw	%xmm7, %xmm2, %xmm2
	vpsubw	%xmm2, %xmm0, %xmm2
	vpmovzxwd	%xmm2, %ymm2            # ymm2 = xmm2[0],zero,xmm2[1],zero,xmm2[2],zero,xmm2[3],zero,xmm2[4],zero,xmm2[5],zero,xmm2[6],zero,xmm2[7],zero
	vcvtdq2ps	%ymm2, %ymm2
	vfmadd213ps	%ymm5, %ymm13, %ymm2    # ymm2 = (ymm13 * ymm2) + ymm5
	vmovaps	%ymm2, (%rbx,%r14)
	vpmulhuw	%xmm0, %xmm8, %xmm2
	vpsrlw	$4, %xmm2, %xmm2
	vpmullw	%xmm2, %xmm9, %xmm2
	vpsubw	%xmm2, %xmm0, %xmm2
	vpaddw	%xmm2, %xmm10, %xmm2
	vpmovzxwd	%xmm2, %ymm2            # ymm2 = xmm2[0],zero,xmm2[1],zero,xmm2[2],zero,xmm2[3],zero,xmm2[4],zero,xmm2[5],zero,xmm2[6],zero,xmm2[7],zero
	vcvtdq2ps	%ymm2, %ymm2
	vmovaps	%ymm2, (%rbx,%r15)
	vmovdqa	%ymm14, (%rbx,%r13)
	vmovaps	%ymm15, (%rbx,%rdi)
	vmovapd	%ymm12, (%rbx,%r12)
	vpaddd	%ymm3, %ymm1, %ymm1
	vpaddw	%xmm0, %xmm11, %xmm0
	addq	$32, %rbx
	addq	$64, %rax
	cmpq	$40000, %rbx                    # imm = 0x9C40
	jne	.LBB2_17
# %bb.18:
	vcvtsi2sdq	80(%rsp), %xmm9, %xmm0  # 8-byte Folded Reload
	vmovsd	__real@412e848000000000(%rip), %xmm8 # xmm8 = [1.0E+6,0.0E+0]
	vmulsd	%xmm0, %xmm8, %xmm0
	vcvtsi2sdq	88(%rsp), %xmm10, %xmm1 # 8-byte Folded Reload
	vdivsd	%xmm1, %xmm0, %xmm0
	vsubsd	96(%rsp), %xmm0, %xmm0          # 8-byte Folded Reload
	vmovsd	__real@408f400000000000(%rip), %xmm1 # xmm1 = [1.0E+3,0.0E+0]
	vdivsd	%xmm1, %xmm0, %xmm6
	vmulsd	%xmm1, %xmm6, %xmm0
	vdivsd	__real@40c3880000000000(%rip), %xmm0, %xmm7
	vpxor	%xmm11, %xmm11, %xmm11
	leaq	144(%rsp), %rax
	vmovdqa	%xmm11, (%rax)
	leaq	56(%rsp), %rcx
	vzeroupper
	callq	*__imp_QueryPerformanceFrequency(%rip)
	leaq	72(%rsp), %rcx
	callq	*__imp_QueryPerformanceCounter(%rip)
	vcvtsi2sdq	72(%rsp), %xmm10, %xmm0
	vmulsd	%xmm0, %xmm8, %xmm0
	vcvtsi2sdq	56(%rsp), %xmm10, %xmm1
	vdivsd	%xmm1, %xmm0, %xmm9
	xorl	%r14d, %r14d
	vxorpd	%xmm8, %xmm8, %xmm8
	leaq	128(%rsp), %r15
	leaq	g_handles(%rip), %r12
	vmovd	__real@3f666666(%rip), %xmm10   # xmm10 = [8.99999976E-1,0.0E+0,0.0E+0,0.0E+0]
	jmp	.LBB2_21
.LBB2_19:                               #   in Loop: Header=BB2_21 Depth=1
	vxorpd	%xmm0, %xmm0, %xmm0
.LBB2_20:                               #   in Loop: Header=BB2_21 Depth=1
	vaddsd	%xmm0, %xmm8, %xmm8
	leaq	144(%rsp), %rax
	vmovdqa	%xmm11, (%rax)
	incl	%r14d
	cmpl	$1000, %r14d                    # imm = 0x3E8
	je	.LBB2_31
.LBB2_21:                               # =>This Loop Header: Depth=1
                                        #     Child Loop BB2_24 Depth 2
	leaq	52(%rsp), %rax
	movq	%rax, 32(%rsp)
	leaq	56(%rsp), %rcx
	leaq	.L__const.main.pos_col(%rip), %rdx
	leaq	.L__const.main.vel_col(%rip), %r8
	movq	%r15, %r9
	callq	engine_ballistics_step_ballistics_10k
	cmpb	$0, 56(%rsp)
	je	.LBB2_30
# %bb.22:                               #   in Loop: Header=BB2_21 Depth=1
	movq	128(%rsp), %rbx
	movq	152(%rsp), %rax
	testq	%rax, %rax
	je	.LBB2_19
# %bb.23:                               #   in Loop: Header=BB2_21 Depth=1
	vxorpd	%xmm0, %xmm0, %xmm0
	movq	%rbx, %rcx
	.p2align	4
.LBB2_24:                               #   Parent Loop BB2_21 Depth=1
                                        # =>  This Inner Loop Header: Depth=2
	movl	(%rcx), %edx
	cmpq	$9999, %rdx                     # imm = 0x270F
	ja	.LBB2_27
# %bb.25:                               #   in Loop: Header=BB2_24 Depth=2
	movl	4(%r12,%rdx,8), %r8d
	cmpl	4(%rcx), %r8d
	jne	.LBB2_27
# %bb.26:                               #   in Loop: Header=BB2_24 Depth=2
	vmulss	(%r13,%rdx,4), %xmm10, %xmm1
	vmovss	%xmm1, (%r13,%rdx,4)
	vmulss	(%rdi,%rdx,4), %xmm10, %xmm1
	vmovss	%xmm1, (%rdi,%rdx,4)
.LBB2_27:                               #   in Loop: Header=BB2_24 Depth=2
	vaddss	16(%rcx), %xmm0, %xmm0
	addq	$20, %rcx
	decq	%rax
	jne	.LBB2_24
# %bb.28:                               #   in Loop: Header=BB2_21 Depth=1
	vcvtss2sd	%xmm0, %xmm0, %xmm0
	jmp	.LBB2_20
.LBB2_29:
	leaq	"??_C@_0CO@LKPMJCNO@?$FLFATAL?$FN?5Simulation?5Fault?5Tag?3?5?$CFd@"(%rip), %rcx
                                        # kill: def $edx killed $edx killed $rdx
	movl	%ebx, %r8d
	callq	printf
.LBB2_30:
	movl	$1, %eax
.LBB2_40:
                                        # kill: def $eax killed $eax killed $rax
	vmovaps	208(%rbp), %xmm6                # 16-byte Reload
	vmovaps	224(%rbp), %xmm7                # 16-byte Reload
	vmovaps	240(%rbp), %xmm8                # 16-byte Reload
	vmovaps	256(%rbp), %xmm9                # 16-byte Reload
	vmovaps	272(%rbp), %xmm10               # 16-byte Reload
	vmovaps	288(%rbp), %xmm11               # 16-byte Reload
	vmovaps	304(%rbp), %xmm12               # 16-byte Reload
	vmovaps	320(%rbp), %xmm13               # 16-byte Reload
	vmovaps	336(%rbp), %xmm14               # 16-byte Reload
	vmovaps	352(%rbp), %xmm15               # 16-byte Reload
	.seh_startepilogue
	leaq	376(%rbp), %rsp
	popq	%rbx
	popq	%rdi
	popq	%rsi
	popq	%r12
	popq	%r13
	popq	%r14
	popq	%r15
	popq	%rbp
	.seh_endepilogue
	retq
.LBB2_31:
	leaq	56(%rsp), %rcx
	callq	*__imp_QueryPerformanceFrequency(%rip)
	leaq	72(%rsp), %rcx
	callq	*__imp_QueryPerformanceCounter(%rip)
	vxorps	%xmm3, %xmm3, %xmm3
	vcvtsi2sdq	72(%rsp), %xmm3, %xmm0
	vxorps	%xmm3, %xmm3, %xmm3
	vcvtsi2sdq	56(%rsp), %xmm3, %xmm1
	movl	$1112014848, g_vx(%rip)         # imm = 0x42480000
	movabsq	$4290672328704, %rax            # imm = 0x3E700000000
	movq	%rax, (%rbx)
	movq	$0, 8(%rbx)
	movabsq	$4294964093648896, %rax         # imm = 0xF423F41200000
	movq	%rax, 16(%rbx)
	movl	$1, 24(%rbx)
	movq	$0, 28(%rbx)
	movl	$1092616192, 36(%rbx)           # imm = 0x41200000
	xorl	%eax, %eax
	testb	%al, %al
	jne	.LBB2_34
# %bb.32:
	movl	g_handles+4(%rip), %eax
	cmpl	4(%rbx), %eax
	jne	.LBB2_34
# %bb.33:
	vmulss	g_vx(%rip), %xmm10, %xmm2
	vmovss	%xmm2, g_vx(%rip)
	vmulss	g_vz(%rip), %xmm10, %xmm2
	vmovss	%xmm2, g_vz(%rip)
.LBB2_34:
	movl	20(%rbx), %ecx
	cmpq	$9999, %rcx                     # imm = 0x270F
	movl	$1, %eax
	ja	.LBB2_37
# %bb.35:
	movl	4(%r12,%rcx,8), %edx
	cmpl	24(%rbx), %edx
	jne	.LBB2_37
# %bb.36:
	vmulss	(%r13,%rcx,4), %xmm10, %xmm2
	vmovss	%xmm2, (%r13,%rcx,4)
	vmulss	(%rdi,%rcx,4), %xmm10, %xmm2
	vmovss	%xmm2, (%rdi,%rcx,4)
.LBB2_37:
	vmovss	g_vx(%rip), %xmm2               # xmm2 = mem[0],zero,zero,zero
	vucomiss	__real@42480000(%rip), %xmm2
	jne	.LBB2_40
	jp	.LBB2_40
# %bb.38:
	vxorps	%xmm2, %xmm2, %xmm2
	vucomisd	%xmm2, %xmm8
	jbe	.LBB2_40
# %bb.39:
	vmulsd	__real@412e848000000000(%rip), %xmm0, %xmm0
	vdivsd	%xmm1, %xmm0, %xmm0
	vsubsd	%xmm9, %xmm0, %xmm0
	vdivsd	__real@408f400000000000(%rip), %xmm0, %xmm9
	leaq	"??_C@_0JN@MKGPCCMF@?$FLCHECK?53?5PASS?$FN?5ReadFrameArena?5Co@"(%rip), %rcx
	vmovapd	%xmm8, %xmm1
	vmovq	%xmm8, %rdx
	callq	printf
	leaq	.Lstr.2(%rip), %rcx
	callq	puts
	leaq	"??_C@_0DH@FLIDPBFL@?5?5Entities?3?5?$CFd?5?$HM?5Frames?3?5?$CFd?5?$CI10?$FO@"(%rip), %rcx
	movl	$10000, %edx                    # imm = 0x2710
	movl	$1000, %r8d                     # imm = 0x3E8
	callq	printf
	leaq	.Lstr.3(%rip), %rcx
	callq	puts
	leaq	"??_C@_0GA@NJMALDLA@?5?5Producer?5Only?5?$CIstep_ballistics@"(%rip), %rcx
	vmovapd	%xmm6, %xmm1
	vmovq	%xmm6, %rdx
	vmovapd	%xmm6, %xmm2
	vmovq	%xmm6, %r8
	vmovapd	%xmm7, %xmm3
	vmovq	%xmm7, %r9
	callq	printf
	vsubsd	%xmm6, %xmm9, %xmm3
	leaq	"??_C@_0HH@IDHFFMPP@?5?5Coupled?5Pipeline?5?$CIProducer?5?$CL?5C@"(%rip), %rcx
	vmovapd	%xmm9, %xmm1
	vmovq	%xmm9, %rdx
	vmovapd	%xmm9, %xmm2
	vmovq	%xmm9, %r8
	vmovq	%xmm3, %r9
	callq	printf
	vmovq	%rsi, %xmm0
	vpunpckldq	__xmm@00000000000000004530000043300000(%rip), %xmm0, %xmm0 # xmm0 = xmm0[0],mem[0],xmm0[1],mem[1]
	vsubpd	__xmm@45300000000000004330000000000000(%rip), %xmm0, %xmm0
	vshufpd	$1, %xmm0, %xmm0, %xmm1         # xmm1 = xmm0[1,0]
	vaddsd	%xmm0, %xmm1, %xmm0
	vmulsd	__real@4059000000000000(%rip), %xmm0, %xmm0
	vdivsd	__real@416312d000000000(%rip), %xmm0, %xmm2
	leaq	"??_C@_0FN@MDBLGLNO@?5?5Total?5Impacts?5Recorded?3?5?5?5?5?$CFll@"(%rip), %rcx
	movq	%rsi, %rdx
	vmovq	%xmm2, %r8
	xorl	%r9d, %r9d
	callq	printf
	xorl	%eax, %eax
	jmp	.LBB2_40
	.seh_endproc
                                        # -- End function
	.def	printf;
	.scl	2;
	.type	32;
	.endef
	.section	.text,"xr",discard,printf
	.globl	printf                          # -- Begin function printf
	.p2align	4
printf:                                 # @printf
.seh_proc printf
# %bb.0:
	pushq	%rsi
	.seh_pushreg %rsi
	pushq	%rdi
	.seh_pushreg %rdi
	pushq	%rbx
	.seh_pushreg %rbx
	subq	$48, %rsp
	.seh_stackalloc 48
	.seh_endprologue
	movq	%rcx, %rsi
	movq	%rdx, 88(%rsp)
	movq	%r8, 96(%rsp)
	movq	%r9, 104(%rsp)
	leaq	88(%rsp), %rbx
	movq	%rbx, 40(%rsp)
	movl	$1, %ecx
	callq	__acrt_iob_func
	movq	%rax, %rdi
	callq	__local_stdio_printf_options
	movq	(%rax), %rcx
	movq	%rbx, 32(%rsp)
	movq	%rdi, %rdx
	movq	%rsi, %r8
	xorl	%r9d, %r9d
	callq	__stdio_common_vfprintf
	nop
	.seh_startepilogue
	addq	$48, %rsp
	popq	%rbx
	popq	%rdi
	popq	%rsi
	.seh_endepilogue
	retq
	.seh_endproc
                                        # -- End function
	.def	__local_stdio_printf_options;
	.scl	2;
	.type	32;
	.endef
	.section	.text,"xr",discard,__local_stdio_printf_options
	.globl	__local_stdio_printf_options    # -- Begin function __local_stdio_printf_options
	.p2align	4
__local_stdio_printf_options:           # @__local_stdio_printf_options
# %bb.0:
	leaq	__local_stdio_printf_options._OptionsStorage(%rip), %rax
	retq
                                        # -- End function
	.section	.rdata,"dr",discard,"??_C@_0IM@ECCBDHPO@?$FLCHECK?50?5PASS?$FN?5Compact?5SoA?5Descr@"
	.globl	"??_C@_0IM@ECCBDHPO@?$FLCHECK?50?5PASS?$FN?5Compact?5SoA?5Descr@" # @"??_C@_0IM@ECCBDHPO@?$FLCHECK?50?5PASS?$FN?5Compact?5SoA?5Descr@"
"??_C@_0IM@ECCBDHPO@?$FLCHECK?50?5PASS?$FN?5Compact?5SoA?5Descr@":
	.asciz	"[CHECK 0 PASS] Compact SoA Descriptors: sizeof(Transform_soa_column_t)=%zuB, sizeof(BallisticVelocity_soa_column_t)=%zuB (1 L1 Cache Line)\n"

	.lcomm	g_handles,80000,64              # @g_handles
	.lcomm	g_px,40000,64                   # @g_px
	.lcomm	g_py,40000,64                   # @g_py
	.lcomm	g_pz,40000,64                   # @g_pz
	.lcomm	g_vx,40000,64                   # @g_vx
	.lcomm	g_vy,40000,64                   # @g_vy
	.lcomm	g_vz,40000,64                   # @g_vz
	.lcomm	g_drag,40000,64                 # @g_drag
	.section	.rdata,"dr"
	.p2align	6, 0x0                          # @__const.main.bad_vel
.L__const.main.bad_vel:
	.quad	4096                            # 0x1000
	.quad	g_handles
	.quad	g_vx
	.quad	g_vy
	.quad	g_vz
	.quad	g_drag
	.zero	16

	.lcomm	g_frame_arena_buf,41024,64      # @g_frame_arena_buf
	.p2align	6, 0x0                          # @__const.main.frame_arena
.L__const.main.frame_arena:
	.quad	g_frame_arena_buf
	.quad	41024                           # 0xa040
	.quad	0                               # 0x0
	.quad	0                               # 0x0
	.zero	32

	.p2align	6, 0x0                          # @__const.main.pos_col
.L__const.main.pos_col:
	.quad	10000                           # 0x2710
	.quad	g_handles
	.quad	g_px
	.quad	g_py
	.quad	g_pz
	.zero	24

	.p2align	6, 0x0                          # @__const.main.vel_col
.L__const.main.vel_col:
	.quad	10000                           # 0x2710
	.quad	g_handles
	.quad	g_vx
	.quad	g_vy
	.quad	g_vz
	.quad	g_drag
	.zero	16

	.p2align	6, 0x0                          # @__const.main.tiny_arena
.L__const.main.tiny_arena:
	.quad	g_frame_arena_buf
	.quad	320                             # 0x140
	.quad	0                               # 0x0
	.quad	0                               # 0x0
	.zero	32

	.section	.rdata,"dr",discard,"??_C@_0CO@LKPMJCNO@?$FLFATAL?$FN?5Simulation?5Fault?5Tag?3?5?$CFd@"
	.globl	"??_C@_0CO@LKPMJCNO@?$FLFATAL?$FN?5Simulation?5Fault?5Tag?3?5?$CFd@" # @"??_C@_0CO@LKPMJCNO@?$FLFATAL?$FN?5Simulation?5Fault?5Tag?3?5?$CFd@"
"??_C@_0CO@LKPMJCNO@?$FLFATAL?$FN?5Simulation?5Fault?5Tag?3?5?$CFd@":
	.asciz	"[FATAL] Simulation Fault Tag: %d at frame %d\n"

	.section	.rdata,"dr",discard,"??_C@_0JN@MKGPCCMF@?$FLCHECK?53?5PASS?$FN?5ReadFrameArena?5Co@"
	.globl	"??_C@_0JN@MKGPCCMF@?$FLCHECK?53?5PASS?$FN?5ReadFrameArena?5Co@" # @"??_C@_0JN@MKGPCCMF@?$FLCHECK?53?5PASS?$FN?5ReadFrameArena?5Co@"
"??_C@_0JN@MKGPCCMF@?$FLCHECK?53?5PASS?$FN?5ReadFrameArena?5Co@":
	.asciz	"[CHECK 3 PASS] ReadFrameArena Consumer Guard blocked Stale Handle (gen=999!=1) & OOB Index (999999>=10000) | Total Dissipated Energy (1,000 frames): %.1f J\n"

	.section	.rdata,"dr",discard,"??_C@_0DH@FLIDPBFL@?5?5Entities?3?5?$CFd?5?$HM?5Frames?3?5?$CFd?5?$CI10?$FO@"
	.globl	"??_C@_0DH@FLIDPBFL@?5?5Entities?3?5?$CFd?5?$HM?5Frames?3?5?$CFd?5?$CI10?$FO@" # @"??_C@_0DH@FLIDPBFL@?5?5Entities?3?5?$CFd?5?$HM?5Frames?3?5?$CFd?5?$CI10?$FO@"
"??_C@_0DH@FLIDPBFL@?5?5Entities?3?5?$CFd?5?$HM?5Frames?3?5?$CFd?5?$CI10?$FO@":
	.asciz	"  Entities: %d | Frames: %d (10^7 total integrations)\n"

	.section	.rdata,"dr",discard,"??_C@_0GA@NJMALDLA@?5?5Producer?5Only?5?$CIstep_ballistics@"
	.globl	"??_C@_0GA@NJMALDLA@?5?5Producer?5Only?5?$CIstep_ballistics@" # @"??_C@_0GA@NJMALDLA@?5?5Producer?5Only?5?$CIstep_ballistics@"
"??_C@_0GA@NJMALDLA@?5?5Producer?5Only?5?$CIstep_ballistics@":
	.asciz	"  Producer Only (step_ballistics_10k):      %.3f ms total | %.3f us/frame (%.3f ns/projectile)\n"

	.section	.rdata,"dr",discard,"??_C@_0HH@IDHFFMPP@?5?5Coupled?5Pipeline?5?$CIProducer?5?$CL?5C@"
	.globl	"??_C@_0HH@IDHFFMPP@?5?5Coupled?5Pipeline?5?$CIProducer?5?$CL?5C@" # @"??_C@_0HH@IDHFFMPP@?5?5Coupled?5Pipeline?5?$CIProducer?5?$CL?5C@"
"??_C@_0HH@IDHFFMPP@?5?5Coupled?5Pipeline?5?$CIProducer?5?$CL?5C@":
	.asciz	"  Coupled Pipeline (Producer + Consumer):   %.3f ms total | %.3f us/frame (%.3f us/frame consumer cost for ~268 hits)\n"

	.section	.rdata,"dr",discard,"??_C@_0FN@MDBLGLNO@?5?5Total?5Impacts?5Recorded?3?5?5?5?5?$CFll@"
	.globl	"??_C@_0FN@MDBLGLNO@?5?5Total?5Impacts?5Recorded?3?5?5?5?5?$CFll@" # @"??_C@_0FN@MDBLGLNO@?5?5Total?5Impacts?5Recorded?3?5?5?5?5?$CFll@"
"??_C@_0FN@MDBLGLNO@?5?5Total?5Impacts?5Recorded?3?5?5?5?5?$CFll@":
	.asciz	"  Total Impacts Recorded:    %llu (%.2f%% impact rate/frame) | Arena Offset Post-Reset: %zu\n"

	.lcomm	__local_stdio_printf_options._OptionsStorage,8,8 # @__local_stdio_printf_options._OptionsStorage
	.section	.rdata,"dr"
.Lstr:                                  # @str
	.asciz	"[CHECK 1 PASS] SoA Cardinality Guard blocked Out-of-Bounds (pos=10000 != vel=4096) -> SimFault_ColumnLengthMismatch"

.Lstr.1:                                # @str.1
	.asciz	"[CHECK 2 PASS] Zero Torn World State: all 10,000 entities completed ground clamping & bounce despite FrameArena overflow -> SimFault_ArenaOverflow"

.Lstr.2:                                # @str.2
	.asciz	"[RustTS v0.9.0-rts Zero-Overhead AVX2 SoA Benchmark]"

.Lstr.3:                                # @str.3
	.asciz	"  Optimizations:             64B Descriptor + assume_safety + WAR-Safe Mirrored SoA + Generational ReadFrameArena"

	.section	.drectve,"yni"
	.ascii	" /DEFAULTLIB:uuid.lib"
	.ascii	" /DEFAULTLIB:uuid.lib"
	.section	.debug$S,"dr"
	.p2align	2, 0x0
	.long	4                               # Debug section magic
	.long	241
	.long	.Ltmp1-.Ltmp0                   # Subsection size
.Ltmp0:
	.short	.Ltmp3-.Ltmp2                   # Record length
.Ltmp2:
	.short	4353                            # Record kind: S_OBJNAME
	.long	0                               # Signature
	.byte	0                               # Object name
	.p2align	2, 0x0
.Ltmp3:
	.short	.Ltmp5-.Ltmp4                   # Record length
.Ltmp4:
	.short	4412                            # Record kind: S_COMPILE3
	.long	0                               # Flags and language
	.short	208                             # CPUType
	.short	22                              # Frontend version
	.short	1
	.short	3
	.short	0
	.short	22013                           # Backend version
	.short	0
	.short	0
	.short	0
	.asciz	"clang version 22.1.3 (https://github.com/llvm/llvm-project e9846648fd6183ee6d8cbdb4502213fcf902a211)" # Null-terminated compiler version string
	.p2align	2, 0x0
.Ltmp5:
.Ltmp1:
	.p2align	2, 0x0
	.addrsig
	.addrsig_sym g_handles
	.addrsig_sym g_px
	.addrsig_sym g_py
	.addrsig_sym g_pz
	.addrsig_sym g_vx
	.addrsig_sym g_vy
	.addrsig_sym g_vz
	.addrsig_sym g_drag
	.addrsig_sym g_frame_arena_buf
	.addrsig_sym __local_stdio_printf_options._OptionsStorage
	.globl	_fltused
