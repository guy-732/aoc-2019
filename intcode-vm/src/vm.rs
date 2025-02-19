use std::str::FromStr;

use num::{Integer, ToPrimitive};

use crate::{
    error::{self, VMError},
    memory::Memory,
};

/// A [VM](IntcodeVM) will return a variant of this enum when it encounters some instructions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VMResult<T> {
    /// Encountered opcode: 99
    ///
    /// Calling [`vm.run()`](IntcodeVM::run) again would simply halt immediatly again
    Halted,
    /// Encountered opcode: 03
    ///
    /// Need to provide an input value to the vm ([`vm.set_next_input()`](IntcodeVM::set_next_input))
    /// before calling [`vm.run()`](IntcodeVM::run) again
    WaitingForInput,
    /// Encoutered opcode: 04
    ///
    /// You can call [`vm.run()`](IntcodeVM::run) again without changing anything.
    /// The associated value of this variant is the output generated by the instruction.
    Output(T),
}

#[derive(Debug, Clone)]
pub struct IntcodeVM<T>
where
    T: Integer + Clone + ToPrimitive,
{
    memory: Memory<T>,
    instruction_ptr: usize,
    relative_base_ptr: T,
    next_input_value: Option<T>,
}

impl<T> IntcodeVM<T>
where
    T: Integer + Clone + ToPrimitive,
{
    /// Creates a new VM from the given [`memory`](Memory)
    ///
    /// # Example
    ///
    /// ```
    /// # use intcode_vm::IntcodeVM;
    /// let vm = IntcodeVM::new([1, 0, 0, 3, 99]);
    /// ```
    #[inline]
    pub fn new<I: Into<Memory<T>>>(memory: I) -> Self {
        Self {
            memory: memory.into(),
            instruction_ptr: 0,
            relative_base_ptr: T::zero(),
            next_input_value: None,
        }
    }

    /// Executes the intcode program in the memory of the VM
    ///
    /// When a halt instruction is encountered, returns [`Ok(VMResult::Halted)`](VMResult::Halted)
    ///
    /// # Examples
    ///
    /// ```
    /// # use intcode_vm::{IntcodeVM, VMResult};
    /// let mut vm = IntcodeVM::new([1, 0, 0, 3, 99]);
    /// assert_eq!(vm.run().unwrap(), VMResult::Halted);
    /// ```
    ///
    /// Will return a [VMError] if a problem occurred, such as an unrecognized op code
    /// ```
    /// # use intcode_vm::IntcodeVM;
    /// let mut vm = IntcodeVM::new([15]); // 15 is not a valid op code
    /// assert!(vm.run().is_err());
    /// ```
    #[inline]
    pub fn run(&mut self) -> error::Result<VMResult<T>, T> {
        loop {
            let instruction = instr::Instruction::from_current_instr_ptr(self)?;
            let instruction_width = instruction.instruction_width();
            match instruction {
                instr::Instruction::Add(arg1, arg2, dest) => {
                    let arg1_val = arg1.resolve_value(self)?;
                    let arg2_val = arg2.resolve_value(self)?;
                    let destination_addr = dest.resolve_address(self)?;

                    let result = arg1_val.clone() + arg2_val.clone();
                    self.memory.set(destination_addr, result);
                    self.increment_instr_ptr_by(instruction_width);
                }

                instr::Instruction::Mul(arg1, arg2, dest) => {
                    let arg1_val = arg1.resolve_value(self)?;
                    let arg2_val = arg2.resolve_value(self)?;
                    let destination_addr = dest.resolve_address(self)?;

                    let result = arg1_val.clone() * arg2_val.clone();
                    self.memory.set(destination_addr, result);
                    self.increment_instr_ptr_by(instruction_width);
                }

                instr::Instruction::ReadInput(dest) => {
                    let destination_addr = dest.resolve_address(self)?;
                    if let Some(input) = self.next_input_value.take() {
                        self.memory.set(destination_addr, input);
                        self.increment_instr_ptr_by(instruction_width);
                    } else {
                        return Ok(VMResult::WaitingForInput);
                    }
                }

                instr::Instruction::WriteOutput(arg) => {
                    let res = arg.resolve_value(self)?.clone();
                    self.increment_instr_ptr_by(instruction_width);
                    return Ok(VMResult::Output(res));
                }

                instr::Instruction::JmpIfTrue(arg, target) => {
                    if !arg.resolve_value(self)?.is_zero() {
                        let target_value = target.resolve_value(self)?;
                        let new_instr_ptr = target_value
                            .to_usize()
                            .ok_or_else(|| VMError::CannotCastToUsize(target_value.clone()))?;

                        self.instruction_ptr = new_instr_ptr;
                    } else {
                        self.increment_instr_ptr_by(instruction_width);
                    }
                }

                instr::Instruction::JmpIfFalse(arg, target) => {
                    if arg.resolve_value(self)?.is_zero() {
                        let target_value = target.resolve_value(self)?;
                        let new_instr_ptr = target_value
                            .to_usize()
                            .ok_or_else(|| VMError::CannotCastToUsize(target_value.clone()))?;

                        self.instruction_ptr = new_instr_ptr;
                    } else {
                        self.increment_instr_ptr_by(instruction_width);
                    }
                }

                instr::Instruction::LessThan(arg1, arg2, result) => {
                    let arg1_val = arg1.resolve_value(self)?;
                    let arg2_val = arg2.resolve_value(self)?;
                    let dest = result.resolve_address(self)?;
                    if arg1_val < arg2_val {
                        self.memory.set(dest, T::one());
                    } else {
                        self.memory.set(dest, T::zero());
                    }

                    self.increment_instr_ptr_by(instruction_width);
                }

                instr::Instruction::Equals(arg1, arg2, result) => {
                    let arg1_val = arg1.resolve_value(self)?;
                    let arg2_val = arg2.resolve_value(self)?;
                    let dest = result.resolve_address(self)?;
                    if arg1_val == arg2_val {
                        self.memory.set(dest, T::one());
                    } else {
                        self.memory.set(dest, T::zero());
                    }

                    self.increment_instr_ptr_by(instruction_width);
                }

                instr::Instruction::AddRelativeBase(arg) => {
                    let arg_val = arg.resolve_value(self)?;
                    self.relative_base_ptr = self.relative_base_ptr.clone() + arg_val.clone();

                    self.increment_instr_ptr_by(instruction_width);
                }

                instr::Instruction::Halt => return Ok(VMResult::Halted),
            }
        }
    }

    /// Returns the internal [Memory] of the VM
    ///
    /// # Example
    ///
    /// ```
    /// # use intcode_vm::IntcodeVM;
    /// let vm = IntcodeVM::from([1, 0, 0, 3, 99]);
    /// let memory = vm.into_memory();
    ///
    /// assert!(memory.memory_starts_with([1, 0, 0, 3, 99].iter()));
    /// ```
    #[inline]
    pub fn into_memory(self) -> Memory<T> {
        self.memory
    }

    #[inline]
    pub const fn get_next_input(&self) -> &Option<T> {
        &self.next_input_value
    }

    #[inline]
    pub fn set_next_input(&mut self, next_input: T) -> Option<T> {
        self.next_input_value.replace(next_input)
    }

    #[inline]
    fn increment_instr_ptr_by(&mut self, incr: usize) {
        self.instruction_ptr += incr;
    }

    #[inline]
    fn get_at_instr_ptr(&self, offset: usize) -> &T {
        self.memory.get(self.instruction_ptr + offset)
    }

    #[inline]
    fn get_3_after_intr_ptr(&self) -> (&T, &T, &T) {
        (
            self.get_at_instr_ptr(1),
            self.get_at_instr_ptr(2),
            self.get_at_instr_ptr(3),
        )
    }

    #[inline]
    fn get_2_after_intr_ptr(&self) -> (&T, &T) {
        (self.get_at_instr_ptr(1), self.get_at_instr_ptr(2))
    }
}

impl<T, I> From<I> for IntcodeVM<T>
where
    T: Integer + Clone + ToPrimitive,
    I: Into<Memory<T>>,
{
    #[inline]
    fn from(memory: I) -> Self {
        Self::new(memory)
    }
}

impl<T> FromStr for IntcodeVM<T>
where
    T: Integer + Clone + ToPrimitive + FromStr,
{
    type Err = <Memory<T> as FromStr>::Err;

    /// Parses the string into a instance of [Memory] (as per [`Memory::from_str()`](Memory::from_str))
    /// then build a VM from it
    ///
    /// # Example
    ///
    /// ```
    /// # use intcode_vm::IntcodeVM;
    /// let vm: IntcodeVM<i32> = "1,0,0,3,99".parse().unwrap();
    ///
    /// assert!(vm.into_memory().memory_starts_with([1, 0, 0, 3, 99].iter()));
    /// ```
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<Memory<T>>().map(Self::new)
    }
}

mod instr {
    use num::{Integer, ToPrimitive};

    use crate::{
        error::{self, VMError},
        IntcodeVM,
    };

    #[derive(Debug, Clone, Copy)]
    enum ArgMode {
        Positional,
        Immediate,
        Relative,
    }

    #[derive(Debug, Clone)]
    pub(super) struct ArgInfo<'t, T> {
        opcode: u16,
        arg_num: u8,
        mode: ArgMode,
        value: &'t T,
    }

    impl<'vm, T> ArgInfo<'vm, T>
    where
        T: Integer + Clone + ToPrimitive,
    {
        #[inline]
        pub(super) fn resolve_value(&self, vm: &'vm IntcodeVM<T>) -> error::Result<&'vm T, T> {
            match self.mode {
                ArgMode::Immediate => Ok(self.value),
                ArgMode::Positional => Ok(vm.memory.get(
                    self.value
                        .to_usize()
                        .ok_or_else(|| VMError::CannotCastToUsize(self.value.clone()))?,
                )),
                ArgMode::Relative => {
                    let real_address = self.value.clone() + vm.relative_base_ptr.clone();
                    Ok(vm.memory.get(
                        real_address
                            .to_usize()
                            .ok_or(VMError::CannotCastToUsize(real_address))?,
                    ))
                }
            }
        }

        #[inline]
        pub(super) fn resolve_address(&self, vm: &'vm IntcodeVM<T>) -> error::Result<usize, T> {
            match self.mode {
                ArgMode::Immediate => Err(VMError::ArgModeCannotBeImmediate {
                    opcode: self.opcode,
                    arg_num: self.arg_num,
                }),
                ArgMode::Positional => self
                    .value
                    .to_usize()
                    .ok_or_else(|| VMError::CannotCastToUsize(self.value.clone())),
                ArgMode::Relative => {
                    let real_address = self.value.clone() + vm.relative_base_ptr.clone();
                    real_address
                        .to_usize()
                        .ok_or(VMError::CannotCastToUsize(real_address))
                }
            }
        }
    }

    impl<'t, T> From<(u16, &'t T, ArgMode, u8)> for ArgInfo<'t, T> {
        #[inline]
        fn from(value: (u16, &'t T, ArgMode, u8)) -> Self {
            Self {
                opcode: value.0,
                arg_num: value.3,
                mode: value.2,
                value: value.1,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub(super) enum Instruction<'t, T> {
        Add(ArgInfo<'t, T>, ArgInfo<'t, T>, ArgInfo<'t, T>),
        Mul(ArgInfo<'t, T>, ArgInfo<'t, T>, ArgInfo<'t, T>),
        ReadInput(ArgInfo<'t, T>),
        WriteOutput(ArgInfo<'t, T>),
        JmpIfTrue(ArgInfo<'t, T>, ArgInfo<'t, T>),
        JmpIfFalse(ArgInfo<'t, T>, ArgInfo<'t, T>),
        LessThan(ArgInfo<'t, T>, ArgInfo<'t, T>, ArgInfo<'t, T>),
        Equals(ArgInfo<'t, T>, ArgInfo<'t, T>, ArgInfo<'t, T>),
        AddRelativeBase(ArgInfo<'t, T>),
        Halt,
    }

    impl<'t, T> Instruction<'t, T>
    where
        T: Integer + Clone + ToPrimitive + 't,
    {
        #[inline]
        pub(super) fn from_current_instr_ptr(vm: &'t IntcodeVM<T>) -> error::Result<Self, T> {
            let instr = vm.get_at_instr_ptr(0);
            let op = instr
                .to_u16()
                .ok_or_else(|| VMError::CannotCastToU16(instr.clone()))?;

            let (arg1_mode, arg2_mode, arg3_mode) = Self::get_3_arg_modes(op)?;
            match op % 100 {
                1 => Self::create_add(vm, arg1_mode, arg2_mode, arg3_mode, op),
                2 => Self::create_mul(vm, arg1_mode, arg2_mode, arg3_mode, op),
                3 => Self::create_read_input(vm, arg1_mode, arg2_mode, arg3_mode, op),
                4 => Self::create_write_output(vm, arg1_mode, arg2_mode, arg3_mode, op),
                5 => Self::create_jmp_if_true(vm, arg1_mode, arg2_mode, arg3_mode, op),
                6 => Self::create_jmp_if_false(vm, arg1_mode, arg2_mode, arg3_mode, op),
                7 => Self::create_less_than(vm, arg1_mode, arg2_mode, arg3_mode, op),
                8 => Self::create_equals(vm, arg1_mode, arg2_mode, arg3_mode, op),
                9 => Self::create_add_relative_base(vm, arg1_mode, arg2_mode, arg3_mode, op),
                99 => Ok(Self::Halt),
                other => Err(VMError::UnknownInstruction(other)),
            }
        }

        #[inline]
        pub(super) const fn instruction_width(&self) -> usize {
            match self {
                Self::Add(_, _, _) => 4,
                Self::Mul(_, _, _) => 4,
                Self::ReadInput(_) => 2,
                Self::WriteOutput(_) => 2,
                Self::JmpIfTrue(_, _) => 3,
                Self::JmpIfFalse(_, _) => 3,
                Self::LessThan(_, _, _) => 4,
                Self::Equals(_, _, _) => 4,
                Self::AddRelativeBase(_) => 2,
                Self::Halt => 1,
            }
        }

        #[inline]
        fn create_add(
            vm: &'t IntcodeVM<T>,
            arg1_mode: ArgMode,
            arg2_mode: ArgMode,
            arg3_mode: ArgMode,
            opcode: u16,
        ) -> error::Result<Self, T> {
            let (arg1, arg2, dest) = vm.get_3_after_intr_ptr();
            Ok(Self::Add(
                (opcode, arg1, arg1_mode, 1).into(),
                (opcode, arg2, arg2_mode, 2).into(),
                (opcode, dest, arg3_mode, 3).into(),
            ))
        }

        #[inline]
        fn create_mul(
            vm: &'t IntcodeVM<T>,
            arg1_mode: ArgMode,
            arg2_mode: ArgMode,
            arg3_mode: ArgMode,
            opcode: u16,
        ) -> error::Result<Self, T> {
            let (arg1, arg2, dest) = vm.get_3_after_intr_ptr();
            Ok(Self::Mul(
                (opcode, arg1, arg1_mode, 1).into(),
                (opcode, arg2, arg2_mode, 2).into(),
                (opcode, dest, arg3_mode, 3).into(),
            ))
        }

        #[inline]
        fn create_read_input(
            vm: &'t IntcodeVM<T>,
            arg1_mode: ArgMode,
            _arg2_mode: ArgMode,
            _arg3_mode: ArgMode,
            opcode: u16,
        ) -> error::Result<Self, T> {
            let arg = vm.get_at_instr_ptr(1);
            Ok(Self::ReadInput((opcode, arg, arg1_mode, 1).into()))
        }

        #[inline]
        fn create_write_output(
            vm: &'t IntcodeVM<T>,
            arg1_mode: ArgMode,
            _arg2_mode: ArgMode,
            _arg3_mode: ArgMode,
            opcode: u16,
        ) -> error::Result<Self, T> {
            let arg = vm.get_at_instr_ptr(1);
            Ok(Self::WriteOutput((opcode, arg, arg1_mode, 1).into()))
        }

        #[inline]
        fn create_jmp_if_true(
            vm: &'t IntcodeVM<T>,
            arg1_mode: ArgMode,
            arg2_mode: ArgMode,
            _arg3_mode: ArgMode,
            opcode: u16,
        ) -> error::Result<Self, T> {
            let (arg1, target) = vm.get_2_after_intr_ptr();
            Ok(Self::JmpIfTrue(
                (opcode, arg1, arg1_mode, 1).into(),
                (opcode, target, arg2_mode, 2).into(),
            ))
        }

        #[inline]
        fn create_jmp_if_false(
            vm: &'t IntcodeVM<T>,
            arg1_mode: ArgMode,
            arg2_mode: ArgMode,
            _arg3_mode: ArgMode,
            opcode: u16,
        ) -> error::Result<Self, T> {
            let (arg1, target) = vm.get_2_after_intr_ptr();
            Ok(Self::JmpIfFalse(
                (opcode, arg1, arg1_mode, 1).into(),
                (opcode, target, arg2_mode, 2).into(),
            ))
        }

        #[inline]
        fn create_less_than(
            vm: &'t IntcodeVM<T>,
            arg1_mode: ArgMode,
            arg2_mode: ArgMode,
            arg3_mode: ArgMode,
            opcode: u16,
        ) -> error::Result<Self, T> {
            let (arg1, arg2, dest) = vm.get_3_after_intr_ptr();
            Ok(Self::LessThan(
                (opcode, arg1, arg1_mode, 1).into(),
                (opcode, arg2, arg2_mode, 2).into(),
                (opcode, dest, arg3_mode, 3).into(),
            ))
        }

        #[inline]
        fn create_equals(
            vm: &'t IntcodeVM<T>,
            arg1_mode: ArgMode,
            arg2_mode: ArgMode,
            arg3_mode: ArgMode,
            opcode: u16,
        ) -> error::Result<Self, T> {
            let (arg1, arg2, dest) = vm.get_3_after_intr_ptr();
            Ok(Self::Equals(
                (opcode, arg1, arg1_mode, 1).into(),
                (opcode, arg2, arg2_mode, 2).into(),
                (opcode, dest, arg3_mode, 3).into(),
            ))
        }

        #[inline]
        fn create_add_relative_base(
            vm: &'t IntcodeVM<T>,
            arg1_mode: ArgMode,
            _arg2_mode: ArgMode,
            _arg3_mode: ArgMode,
            opcode: u16,
        ) -> error::Result<Self, T> {
            let arg = vm.get_at_instr_ptr(1);
            Ok(Self::AddRelativeBase((opcode, arg, arg1_mode, 1).into()))
        }

        #[inline]
        fn get_3_arg_modes(opcode: u16) -> Result<(ArgMode, ArgMode, ArgMode), VMError<T>> {
            let mut op = opcode / 100;
            let arg1 = (op % 10) as u8;
            op /= 10;
            let arg2 = (op % 10) as u8;
            op /= 10;
            let arg3 = op as u8;
            Ok((
                Self::parse_arg_mode(opcode, arg1, 1)?,
                Self::parse_arg_mode(opcode, arg2, 2)?,
                Self::parse_arg_mode(opcode, arg3, 3)?,
            ))
        }

        #[inline]
        fn parse_arg_mode(opcode: u16, arg_mode: u8, arg_num: u8) -> error::Result<ArgMode, T> {
            match arg_mode {
                0 => Ok(ArgMode::Positional),
                1 => Ok(ArgMode::Immediate),
                2 => Ok(ArgMode::Relative),
                _ => Err(VMError::InvalidArgMode {
                    opcode,
                    arg_num,
                    arg_mode,
                }),
            }
        }
    }
}
