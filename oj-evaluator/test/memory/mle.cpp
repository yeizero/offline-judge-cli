#include <iostream>
#include <vector>
using namespace std;

int main() {
    std::vector<std::vector<int>> bigData;

    const size_t outerSize = 4000000;
    const size_t innerSize = 100;

    bigData.reserve(outerSize);

    for (size_t i = 0; i < outerSize; ++i) {
        bigData.emplace_back(innerSize, static_cast<int>(i));
    }

    cout << "end";

    return 0;
}
