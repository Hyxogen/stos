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
CC		:= clang
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
CFLAGS		:= -O2 -g $(WARNINGS) -I.. -MMD -MP

CFILES		:= $(shell find . -type f -name '*.c')
HEADER_DEPS	:= $(CFILES:.c=.d)
TSTFILES	:= $(addsuffix .mem.out,$(CFILES)) \
		   $(addsuffix .addr.out,$(CFILES))

BIN_DIR		:= bin

.PHONY:	all
all: run

.PHONY: run
run: $(TSTFILES)
	-@rc=0; count=0; res=0; \
	for file in $(CFILES); do \
		printf "%s\t" $$(basename $$file); \
		./$$file.mem.out; \
		res="$$?"; \
		if [ "$$res" -eq 0 ]; \
		then printf MEMORY_PASSED; \
		else printf MEMORY_FAILED; \
		fi; \
		printf "\t"; \
		rc=`expr $$rc + $$(($$res != 0))`; count=`expr $$count + 1`; \
		./$$file.addr.out; \
		res="$$?"; \
		if [ "$$res" -eq 0 ]; \
		then printf ADDRESS_PASSED; \
		else printf ADDRESS_FAILED; \
		fi; \
		printf "\n"; \
		rc=`expr $$rc + $$(($$res != 0))`; count=`expr $$count + 1`; \
	done; \
	echo; echo "Executed $$count tests	Failed tests $$rc"

.PHONY: testdrivers
testdrivers: $(TSTFILES)

-include $(HEADER_DEPS)
%.c.mem.out: %.c Makefile
	$(CC) $(CFLAGS) -fsanitize=memory -o $@ $<

-include $(HEADER_DEPS)
%.c.addr.out: %.c Makefile
	$(CC) $(CFLAGS) -fsanitize=address,undefined,integer -o $@ $<

.PHONY: clean
clean:
	rm -f $(TSTFILES) $(HEADER_DEPS)
