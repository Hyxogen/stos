# utility makefile for running tests

# Copyright (C) 2022 Daan Meijer
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.
TARGET		:= ass_fuzz

CC		:= clang
LINK_CMD	:= $(CC)

WARNINGS	:= -Wall -Wextra -std=c99 -pedantic \
		   -Wconversion -Wshorten-64-to-32 \
		   -Warray-bounds-pointer-arithmetic -Wimplicit-fallthrough \
		   -Wconditional-uninitialized -Wloop-analysis \
		   -Wshift-sign-overflow -Wswitch-enum \
		   -Wtautological-constant-in-range-compare -Wcomma \
		   -Wassign-enum -Wbad-function-cast -Wfloat-equal \
		   -Wformat-type-confusion -Wpointer-arith \
		   -Widiomatic-parentheses -Wunreachable-code-aggressive \
		   -Wthread-safety
INCLUDE_DIRS	:= -I..
SANITIZERS	:= -fsanitize=address,undefined,integer
CFLAGS		:= -O2 -g $(SANITIZERS) $(WARNINGS) $(INCLUDE_DIRS) -MMD -MP
LFLAGS		:= -flto $(SANITIZERS)

CFILES		:= ass_extract_fuzz.c
HEADER_DEPS	:= $(CFILES:.c=.d)
OBJ_FILES	:= $(CFILES:.c=.o)

BIN_DIR		:= bin

.PHONY:	all
all: $(TARGET)

$(TARGET): $(OBJ_FILES)
	$(LINK_CMD) $(OBJ_FILES) -o $@ $(LFLAGS)

-include $(HEADER_DEPS)
%.o: %.c Makefile
	$(CC) $(CFLAGS) -o $@ -c $<

.PHONY: clean
clean:
	rm -f $(TARGET) $(OBJ_FILES) $(HEADER_DEPS)
