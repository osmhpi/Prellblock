run ```docker-compose up```

After executing this command you can open http://{your-host-ip}:8080 in you browser (for ex. http://localhost:8080). You should see ThingsBoard login page. Use the following default credentials:

    Systen Administrator: sysadmin@thingsboard.org / sysadmin
    Tenant Administrator: tenant@thingsboard.org / tenant
    Customer User: customer@thingsboard.org / customer

Use the **tenant** account to create an manage devices.

You can send value to a device using the following command:
```sh
curl -v -X POST -d "{\"temperature\": 25}" http://localhost:8080/api/v1/$ACCESS_TOKEN/telemetry --header "Content-Type:application/json"
```

You can compile `prellblock` with the `thingsboard` feature. This will enable the communication with a thignsboard instance.

You'll need to set some environment variables to connect to thingboard:
>"THINGSBOARD_USER_NAME"  
*(this is your (tenant) account's name)*

>"THINGSBOARD_PASSWORD"   
*(this ist your (tenant) account's password)*

>"THINGSBOARD_TENANT_ID"  
*(this ist your (tenant) account's id)*

After that, the setup should be automatic.  There won't happen anything, if you do not subscribe to a timeseries in the `../config/subscription_config.toml`.  