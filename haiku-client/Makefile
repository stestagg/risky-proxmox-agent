APP_NAME := risky_proxmox_haiku
SRC := src/main.cpp

CXXFLAGS += -std=c++17 -Wall -Wextra
LDFLAGS += -lbe

$(APP_NAME): $(SRC)
	$(CXX) $(CXXFLAGS) -o $@ $(SRC) $(LDFLAGS)

clean:
	rm -f $(APP_NAME)

.PHONY: clean
