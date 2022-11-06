TARGET		:= stos

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

SOURCE_FILES	:= main.c sub.c
OBJECT_FILES	:= $(SOURCE_FILES:.c=.o)
DEPEND_FILES	:= $(SOURCE_FILES:.c=.d)

CFLAGS		:= -Wall -Wextra -std=c99 -pedantic -MMD -MP \
		   -I. $(WARNINGS)
LFLAGS		:= -Wall -Wextra -std=c99 -pedantic -lavcodec -lavutil \
		   -lavformat

ifndef config
	config	:= debug
endif

ifeq ($(config), debug)
	CFLAGS += -g3 -O0 -fsanitize=address,undefined
	LFLAGS += -g3 -O0 -fsanitize=address,undefined
else ifeq ($(config),distr)
	CFLAGS += -g0 -Ofast -march=native
	LFLAGS += -g0 -Ofast -march=native -flto
endif

all: $(TARGET)

$(TARGET): $(OBJECT_FILES)
	$(CC) -o $(TARGET) $(OBJECT_FILES) $(LFLAGS)

-include $(DEPEND_FILES)
%.o: %.c Makefile
	$(CC) $(CFLAGS) -c $< -o $@

re:
	${MAKE} clean
	${MAKE}

clean:
	rm -f $(OBJECT_FILES)
	rm -f $(DEPEND_FILES)
	rm -f $(TARGET)
