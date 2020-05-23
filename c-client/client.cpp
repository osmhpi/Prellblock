#include <stdlib.h>
#include <iostream>
#include <iomanip>
#include <ctime>
#include <chrono>

#include "../prellblock-client/prellblock-client.h"

#define NUM_TX 10000

int main(void)
{
    std::chrono::system_clock::time_point start = std::chrono::system_clock::now();

    prellblock::Client *client = prellblock::create_client_instance("127.0.0.1:3133", "406ed6170c8672e18707fb7512acf3c9dbfc6e5ad267d9a57b9c486a94d99dcc");

    const char number[10] = {0};
    for (size_t i = 0; i < NUM_TX; i++)
    {
        sprintf((char *)number, "%lu", i);
        prellblock::send_key_value(client, "prellblock", number);
    }

    prellblock::destroy_client_instance(client);

    std::chrono::system_clock::time_point end = std::chrono::system_clock::now();
    const int64_t microseconds = std::chrono::duration_cast<std::chrono::microseconds>(end - start).count();
    const double seconds = (double)microseconds / 1000 / 1000;
    std::cout << "Sending "
              << NUM_TX
              << " transactions took "
              << seconds
              << "s, resulting in "
              << (double)NUM_TX / seconds
              << "TPS."
              << std::endl;

    return 0;
}